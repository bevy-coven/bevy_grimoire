//! **The BRP client** — the MCP server's way into the running editor.
//!
//! **JSON-RPC 2.0 over HTTP POST**, which is what `bevy_remote::http` speaks since Bevy 0.16.
//! Every request is a POST of
//! `{"jsonrpc":"2.0","id":N,"method":…, "params":…}` to the BRP root URL, and the answer is the
//! JSON-RPC response object.
//!
//! **The editor is a separate process and this server never binds a socket.** The editor owns the
//! world; the MCP server asks it questions and tells it what to do. That is the whole reason the
//! editor's verbs are registered as BRP commands rather than being callable in-process.

use serde_json::{Value, json};

/// **A BRP client.**
#[derive(Debug, Clone)]
pub struct Brp {
    url: String,
    /// **The request counter, in an `Arc`** because `AtomicU64` is not `Clone` and the server is
    /// cloned into every tool call.
    next_id: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

/// **How long a read may take.** The editor answers a registered command in a frame or two, so this
/// is already generous; it exists so a dead editor is reported in seconds rather than never.
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// **How long a change may take**, and why it is not the read budget (B-050).
///
/// `grimoire/operation_apply` can parse a project, lay a fill to `MAX_CELLS`, or keep a layout that
/// saves every row. Measured on 2026-10-25 at load average 32: `map_create` on a real project passed
/// five seconds, and the refusal told the agent to check whether the editor was running. A budget
/// that is right for a read is wrong for work.
const MUTATING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// **The methods that do work**, so they get [`MUTATING_TIMEOUT`]. Everything else is a read.
const MUTATING_METHODS: &[&str] = &[
    "grimoire/operation_apply",
    "grimoire/operation_undo",
    "grimoire/capture_sequence_start",
    // **The project guard runs before every mutation, so it is on the mutation's clock.** It is a
    // read of two strings, but it is asked *of an editor that is about to be asked to do work that
    // takes tens of seconds* — and a freshly started editor at load 32 answered it in more than five
    // seconds, which made `placement_add` report a timeout before it had been tried.
    "grimoire/project_root",
];

/// **Why a call failed**, for a caller that treats the two causes differently (#179).
enum Failure {
    /// **The call ran out of its budget**: for the local editor, one that is up and busy.
    Busy(String),
    /// **The transport, the editor's refusal, or an answer that could not be read.**
    Other(String),
}

impl Failure {
    /// **The sentence an agent reads**, whichever the cause.
    fn into_message(self) -> String {
        match self {
            Failure::Busy(m) | Failure::Other(m) => m,
        }
    }
}

impl Brp {
    /// **A client for the editor at `url`.**
    pub fn new(url: impl Into<String>) -> Brp {
        Brp {
            url: url.into(),
            next_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// **The editor's URL, from `BEVY_BRP_PORT`.**
    ///
    /// **The same variable the editor reads** (`harness.rs`: `BEVY_BRP_PORT`, default 15702), so
    /// one knob points both ends at the same socket.
    pub fn from_env() -> Brp {
        let port = std::env::var("BEVY_BRP_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(15702);
        Brp::new(format!("http://127.0.0.1:{port}/"))
    }

    /// **Call one BRP method, without blocking the async runtime.**
    ///
    /// **The editor answers in a frame or two**, and a tool call an agent is waiting on should
    /// not hold up the runtime's other tasks while it waits. **So the call is made on the blocking
    /// pool** — `ureq` is a blocking client, and calling it on the async thread would park the
    /// whole runtime for the length of the request.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.on_blocking_pool(method, params, Self::call_blocking)
            .await?
    }

    /// **Take one picture through `bevy_fairy`, as base64 PNG**, of the editor or of a game: `params` is
    /// `bevy_fairy/screenshot`'s (`{"source": "offscreen"}` for the editor, `{}` for a game's own default).
    ///
    /// **Two calls, because the screenshot is asynchronous.** The answer to `bevy_fairy/screenshot` is an
    /// id, and the picture is ready a frame or two later, when `bevy_fairy/capture` answers with the
    /// bytes. So this asks, waits, and asks again, and the wait is bounded: a capture that never lands
    /// must not hang an agent.
    pub async fn screenshot(&self, params: Value) -> Result<String, String> {
        let asked = self.call("bevy_fairy/screenshot", params).await?;
        let id = asked
            .get("id")
            .and_then(|i| i.as_u64())
            .ok_or("the capture was not given an id")?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let answer = self.call("bevy_fairy/capture", json!({"id": id})).await?;
            match answer.get("status").and_then(|s| s.as_str()) {
                Some("done") => {
                    return answer
                        .get("png_base64")
                        .and_then(|p| p.as_str())
                        .map(|p| p.to_owned())
                        .ok_or_else(|| "the capture had no picture in it".to_owned());
                }
                Some("failed") => {
                    let why = answer.get("error").and_then(|e| e.as_str()).unwrap_or("no reason was given");
                    return Err(format!("the capture failed: {why}"));
                }
                _ => {}
            }
            if std::time::Instant::now() > deadline {
                return Err("the capture did not arrive within ten seconds.".to_owned());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// **Call one BRP method, where an editor too busy to answer within the budget is `Ok(None)`,
    /// not a refusal** (#179).
    ///
    /// For a poll of work the editor is already doing: a status it could not answer in time is
    /// "still running", and the caller decides when to stop asking. A missing editor, a refusal or
    /// an unreadable answer is still an `Err`.
    pub async fn call_unless_busy(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Option<Value>, String> {
        match self
            .on_blocking_pool(method, params, Self::attempt_blocking)
            .await?
        {
            Ok(v) => Ok(Some(v)),
            Err(Failure::Busy(_)) => Ok(None),
            Err(Failure::Other(e)) => Err(e),
        }
    }

    /// **Run one request on the blocking pool**, with the next request id.
    async fn on_blocking_pool<T: Send + 'static>(
        &self,
        method: &str,
        params: Value,
        request: fn(&str, &str, u64, Value) -> T,
    ) -> Result<T, String> {
        let url = self.url.clone();
        let name = method.to_owned();
        let next_id = self.next_id.clone();
        tokio::task::spawn_blocking(move || {
            let id = next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            request(&url, &name, id, params)
        })
        .await
        .map_err(|e| format!("the editor call did not finish: {e}"))
    }

    /// **The call itself, on a thread that is allowed to block.**
    fn call_blocking(url: &str, method: &str, id: u64, params: Value) -> Result<Value, String> {
        Self::attempt_blocking(url, method, id, params).map_err(Failure::into_message)
    }

    /// **The call, with the cause of a failure kept** for [`Brp::call_unless_busy`].
    fn attempt_blocking(url: &str, method: &str, id: u64, params: Value) -> Result<Value, Failure> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        // **A cap, because a tool call an agent is waiting on should not hang — but the cap is not
        // the same for every method** (B-050).
        //
        // Five seconds is a long time to wait for a read that the editor answers in a frame or two,
        // and much too short for work that legitimately parses a project or lays four thousand
        // cells. Measured 2026-10-25 at load average 32: `map_create` on Sanctuary's project
        // exceeded five seconds, and the refusal read *"the editor did not answer ... Is it running,
        // and is BEVY_BRP_PORT the same on both sides?"* — sending an agent to check a process that
        // was up and healthy. **A timeout must not blame the connection.**
        let budget = if MUTATING_METHODS.contains(&method) {
            MUTATING_TIMEOUT
        } else {
            READ_TIMEOUT
        };
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder().timeout_global(Some(budget)).build(),
        );
        let response = agent.post(url).send_json(&body).map_err(|e| {
            // **Two sentences, because there are two causes.** `TimedOut` is the editor taking too
            // long over the work; anything else is the transport. Telling an agent to look at its
            // port for a slow operation is the misattribution this constant exists to end.
            let timed_out = match &e {
                ureq::Error::Timeout(_) => true,
                ureq::Error::Io(io) => io.kind() == std::io::ErrorKind::TimedOut,
                _ => false,
            };
            if timed_out {
                Failure::Busy(format!(
                    "the editor did not finish `{method}` within {} s, so the call was abandoned. \
                     It is running — this is the work taking longer than the budget, not a missing \
                     editor. Ask for less at once, or check grimoire/state to see what it is doing.",
                    budget.as_secs()
                ))
            } else {
                Failure::Other(format!(
                    "the editor did not answer `{method}` at {url}: {e}. Is it running, and is \
                     BEVY_BRP_PORT the same on both sides?"
                ))
            }
        })?;
        let text = response.into_body().read_to_string().map_err(|e| {
            Failure::Other(format!(
                "the editor's answer to `{method}` was not readable: {e}"
            ))
        })?;
        let parsed: Value = serde_json::from_str(&text).map_err(|e| {
            Failure::Other(format!(
                "the editor's answer to `{method}` was not JSON: {e}"
            ))
        })?;
        // **A JSON-RPC error is a refusal, and it is reported as one.**
        if let Some(error) = parsed.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("the editor refused the call");
            // **"Method not found" means the editor is not the one we think it is** — an editor
            // that is not running, or one built without the debugger's command table. **Saying
            // "method not found" would send the agent looking for a typo**, and the answer is
            // that there is no editor to answer.
            if message.contains("not found") {
                return Err(Failure::Other(format!(
                    "the editor at {url} does not answer `{method}` — it is probably not running, \
                     or it was built without its BRP commands. Start it and try again."
                )));
            }
            return Err(Failure::Other(format!("{method}: {message}")));
        }
        parsed
            .get("result")
            .cloned()
            .ok_or_else(|| Failure::Other(format!("{method}: the editor's answer had no result")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A listener that accepts one connection and then says nothing, for `hold`.** The port is
    /// read back from the bound socket, so the test never guesses one.
    fn a_silent_editor(hold: std::time::Duration) -> (u16, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("a bound address").port();
        let held = std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                // **Held open, never written to** — the shape of an editor that is working, not one
                // that is gone.
                std::thread::sleep(hold);
                drop(stream);
            }
        });
        (port, held)
    }

    /// **B-050: a slow editor must not be reported as a missing one.**
    ///
    /// The read budget is five seconds and the stub holds the connection for eight without
    /// answering, so the call times out with the editor plainly up. The failure this pins is not
    /// "it timed out" — it is *what the timeout says*: the refusal must name the work taking
    /// longer, and must not send the agent to check whether the editor is running.
    #[test]
    fn a_slow_editor_is_not_blamed_on_the_connection() {
        let (port, held) = a_silent_editor(std::time::Duration::from_secs(8));
        let started = std::time::Instant::now();
        let answer = Brp::call_blocking(
            &format!("http://127.0.0.1:{port}/"),
            "grimoire/state",
            1,
            json!({}),
        );
        let took = started.elapsed();
        held.join().expect("the stub thread");

        let refusal = answer.expect_err("a stub that never answers must not produce an answer");
        assert!(
            refusal.contains("did not finish `grimoire/state` within 5 s"),
            "the refusal must name the work and its budget, and read: {refusal}"
        );
        assert!(
            !refusal.contains("Is it running"),
            "a slow editor is running — the refusal must not ask, and read: {refusal}"
        );
        assert!(
            took >= READ_TIMEOUT && took < std::time::Duration::from_secs(8),
            "the read budget of 5 s is what abandoned the call, and it took {took:?}"
        );
    }

    /// **B-050's other half: an editor that is not there IS named as missing.**
    ///
    /// Without this, "never blame the connection" would be satisfied by never mentioning the
    /// connection at all — and then a port with nothing behind it would be reported as slow work.
    #[test]
    fn a_missing_editor_is_named_as_missing() {
        // **A port that was bound and released, not one that is held open.** A held-open listener
        // accepts the connection and answers nothing — that is the slow case above; missing is a
        // port with no listener behind it. Held open, this test hung on the stub's own `accept`.
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
            listener.local_addr().expect("a bound address").port()
        };
        let answer = Brp::call_blocking(
            &format!("http://127.0.0.1:{port}/"),
            "grimoire/state",
            1,
            json!({}),
        );
        let refusal = answer.expect_err("nothing is listening, so there must be no answer");
        assert!(
            refusal.contains("Is it running"),
            "a missing editor must be named as one, and read: {refusal}"
        );
    }
}
