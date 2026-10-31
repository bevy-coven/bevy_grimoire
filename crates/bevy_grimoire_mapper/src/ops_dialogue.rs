//! **Walk a conversation without playing it, as one registered operation** (#106): `dialogue_preview`.
//!
//! A read: it loads the project's `DialogueScript` (the crate's own type, from
//! `family/bevy_speech_bubbles.ron`), follows one path through a conversation taking the named
//! options at each choice, and answers with the lines said, where the walk stopped, and which nodes
//! no path from the start reaches. It changes no document, so it has no undo.

use std::collections::{BTreeSet, HashSet};

use bevy_speech_bubbles::{DialogueScript, Node};
use serde::Deserialize;
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub conversation: String,
    /// The option index taken at each choice met, in order. A choice with none left stops the walk.
    #[serde(default)]
    pub choices: Vec<usize>,
}

/// **Walk one path through a conversation.**
pub struct DialoguePreview;

impl DialoguePreview {
    fn run(ctx: &Ctx, raw: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p: Params = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a dialogue preview: {e}. The fields are `conversation` and `choices`.")))?;
        let path = crate::ops_family::settings_path(ctx, "bevy_speech_bubbles")?;
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Refusal::new(format!("{} cannot be read, so there is nothing to preview: {e}", path.display())))?;
        let script: DialogueScript =
            ron::from_str(&text).map_err(|e| Refusal::new(format!("the project's dialogue script is not readable: {e}")))?;
        let Some(conv) = script.conversation(&p.conversation) else {
            let mut known: Vec<&str> = script.conversations.keys().map(String::as_str).collect();
            known.sort_unstable();
            return Err(Refusal::new(format!("the dialogue script has no conversation `{}` (it has: {}).", p.conversation, known.join(", "))));
        };

        let mut lines = Vec::new();
        let mut seen = HashSet::new();
        let mut taken = p.choices.iter();
        let mut at = Some(conv.start.clone());
        let mut waiting = serde_json::Value::Null;
        while let Some(id) = at.take() {
            // A conversation may loop back on itself; the walk stops where it would repeat a node.
            if !seen.insert(id.clone()) {
                break;
            }
            match conv.nodes.get(&id) {
                Some(Node::Line { speaker, text, next, .. }) => {
                    lines.push(json!({"node": id, "speaker": speaker, "text": text}));
                    at = next.clone();
                }
                Some(Node::Choice { speaker, prompt, options, .. }) => {
                    lines.push(json!({"node": id, "speaker": speaker, "text": prompt}));
                    match taken.next() {
                        Some(&i) => match options.get(i) {
                            Some(o) => at = Some(o.next.clone()),
                            None => {
                                return Err(Refusal::new(format!("choice `{id}` has {} options, so option {i} does not exist.", options.len())));
                            }
                        },
                        None => waiting = json!({"node": id, "options": options.iter().map(|o| o.text.as_str()).collect::<Vec<_>>()}),
                    }
                }
                None => return Err(Refusal::new(format!("node `{id}` is not in conversation `{}`.", p.conversation))),
            }
        }

        let mut reach = BTreeSet::new();
        let mut stack = vec![conv.start.as_str()];
        while let Some(id) = stack.pop() {
            if !reach.insert(id) {
                continue;
            }
            match conv.nodes.get(id) {
                Some(Node::Line { next: Some(n), .. }) => stack.push(n),
                Some(Node::Choice { options, .. }) => stack.extend(options.iter().map(|o| o.next.as_str())),
                _ => {}
            }
        }
        let unreachable: BTreeSet<&str> = conv.nodes.keys().map(String::as_str).filter(|k| !reach.contains(k)).collect();
        Ok(json!({
            "lines": lines,
            "ended": waiting.is_null(),
            "waiting_at": waiting,
            "unreachable": unreachable,
        }))
    }
}

impl Operation for DialoguePreview {
    fn name(&self) -> &'static str {
        "dialogue_preview"
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["conversation"],
            "properties": {
                "conversation": {"type": "string", "description": "The conversation's name in the project's dialogue script."},
                "choices": {"type": "array", "items": {"type": "integer", "minimum": 0}, "description": "The option index (from 0) to take at each choice, in the order met. A choice with none left stops the walk and its options are reported."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        Self::run(ctx, params)
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let after = Self::run(ctx, params)?;
        let n = after["lines"].as_array().map_or(0, Vec::len);
        let tail = if after["ended"] == true { "ended" } else { "waiting at a choice" };
        Ok(Outcome { summary: format!("dialogue preview: {n} lines, {tail}"), affected: 0, saved: false, inverse: None, after: Some(after) })
    }
}
