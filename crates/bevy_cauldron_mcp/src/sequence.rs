//! **Waiting for a capture sequence to end.**

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::brp::Brp;

/// **How long between two status polls.**
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// **Poll `grimoire/capture_sequence_status` for sequence `id` until it ends or `deadline` passes.**
///
/// **A poll the editor is too busy to answer within its budget is "still running", not a refusal**
/// (#179). An idle editor answers a status in a frame or two, but on a loaded machine one frame can
/// outlast the read budget while frames are still being written: measured on bmb at load 17, three
/// clips in a row were abandoned on one such poll. So a busy poll is counted, the next one is made,
/// and only the deadline ends the wait:
///
/// - **an answer that is not `running`** (done, or failed with its reason) is returned at once;
/// - **at the deadline**, the newest answer is returned as it stands, with the frames written so
///   far; when no poll was answered at all, the wait refuses and says how many went unanswered.
///
/// A missing editor, or one that refuses the poll, still ends the wait with its refusal.
pub async fn wait_for_sequence(editor: &Brp, id: u64, deadline: Instant) -> Result<Value, String> {
    let mut newest = None;
    let mut busy_polls = 0u32;
    loop {
        match editor
            .call_unless_busy("grimoire/capture_sequence_status", json!({"id": id}))
            .await?
        {
            Some(answer) if answer.get("status").and_then(|s| s.as_str()) != Some("running") => {
                return Ok(answer);
            }
            Some(answer) => newest = Some(answer),
            None => busy_polls += 1,
        }
        if Instant::now() > deadline {
            return newest.ok_or_else(|| {
                format!(
                    "the editor answered none of {busy_polls} status poll(s) for sequence {id} \
                     within the read budget before the sequence's deadline. It is running — this \
                     is the editor too busy to answer, not a missing one. Ask for fewer frames or \
                     a shorter sequence, or check grimoire/state to see what it is doing."
                )
            });
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
