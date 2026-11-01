/// Where decisions are asked. **One endpoint per run**: there is no fallback chain.
///
/// Local clef-flash is the default. Clef 27B (bmb) and Cloudflare Workers AI are values of the same
/// three fields. Cloudflare sends screenshots off the LAN, so it is opt-in: only by setting these.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// The full URL a request is POSTed to.
    pub endpoint: String,
    /// Sent as `model` when set. A llama.cpp server has one model and needs none.
    pub model: Option<String>,
    /// Sent as a bearer token when set.
    pub key: Option<String>,
    pub timeout_secs: u64,
}

/// clef-flash on this LAN: `tools/decide/serve-clef-flash.sh` on big.
pub const LOCAL_ENDPOINT: &str = "http://127.0.0.1:8093/v1/systemone";

impl Default for Config {
    fn default() -> Self {
        Config { endpoint: LOCAL_ENDPOINT.into(), model: None, key: None, timeout_secs: 60 }
    }
}

impl Config {
    /// `GRIMOIRE_DECIDE_ENDPOINT`, `GRIMOIRE_DECIDE_MODEL`, `GRIMOIRE_DECIDE_KEY`, `GRIMOIRE_DECIDE_TIMEOUT_SECS`
    /// over the local default; `get` is the environment, so tests pass a map.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let d = Config::default();
        let set = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        Config {
            endpoint: set("GRIMOIRE_DECIDE_ENDPOINT").unwrap_or(d.endpoint),
            model: set("GRIMOIRE_DECIDE_MODEL"),
            key: set("GRIMOIRE_DECIDE_KEY"),
            timeout_secs: set("GRIMOIRE_DECIDE_TIMEOUT_SECS").and_then(|v| v.parse().ok()).unwrap_or(d.timeout_secs),
        }
    }
    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
    }
}
