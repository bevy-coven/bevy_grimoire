//! **Making a kit and choosing where new work lands, as registered operations** (E-002, #129 slice 20):
//! `kit_create` and `kit_set_authoring`.
//!
//! The project chooser made a kit (`chooser::create_kit`) and pointed `authoring` at one
//! (`chooser::set_authoring`) with writes of its own, so an agent could place pieces and import meshes
//! and never start the kit they belong in. Both now dispatch these from the menu, which has a root and no
//! loaded project, so the log line goes in the root's `oplog.jsonl` (the way `map_create`'s does).
//!
//! **`kit_create` undoes exactly what it wrote and nothing else.** Its inverse names the kit and the
//! text `kits.ron` had before; it takes the kit's directory away only while that holds nothing but the
//! two files this operation wrote, unchanged, and puts `kits.ron` back. A kit someone has since put a
//! piece in, or a file the author touched, is refused rather than deleted. The undo is the writer's own
//! ([`Operation::only_as_undo`]): an agent cannot delete a kit by calling `kit_create`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::chooser::{GRIMOIRE_DIR, new_kit_files};
use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

fn root(ctx: &Ctx) -> Result<PathBuf, Refusal> {
    ctx.root_or_project()
        .ok_or_else(|| Refusal::new("no project is open, so there is no kits.ron to bind a kit in."))
}

fn kits_path(root: &Path) -> PathBuf {
    root.join(GRIMOIRE_DIR).join(bevy_grimoire_core::kits::KITS_FILE)
}

/// What `kits.ron` holds now, for an undo to compare and to put back.
fn kits_text(root: &Path) -> Result<String, Refusal> {
    let path = kits_path(root);
    std::fs::read_to_string(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Refusal::new(format!(
                "{} does not exist — this directory is not a project, so there is nothing to bind a kit in.",
                path.display()
            ))
        } else {
            Refusal::new(format!("cannot read {}: {e}", path.display()))
        }
    })
}

// ── kit_create ───────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateParams {
    /// What the kit is called, forced to snake_case. It is also the namespace its pieces take.
    #[serde(default)]
    pub name: String,
    /// **Undo only.** Take this kit away again, and put `kits.ron` back as `kits` says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<String>,
    /// **Undo only.** The text `kits.ron` had before the kit was bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kits: Option<String>,
}

impl CreateParams {
    fn parse(raw: &serde_json::Value) -> Result<CreateParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("this is not a kit to make: {e}. The only field is `name`."))
        })
    }
}

/// **Make a new, empty kit and bind it**, as the chooser's `+ new kit` row does.
pub struct KitCreate;

impl KitCreate {
    fn dir(root: &Path, name: &str) -> PathBuf {
        root.join(GRIMOIRE_DIR).join(name)
    }

    fn removal(ctx: &Ctx, p: &CreateParams, write: bool) -> Result<Outcome, Refusal> {
        let root = root(ctx)?;
        let (Some(name), Some(prior)) = (&p.remove, &p.kits) else {
            return Err(Refusal::new("an undo names the kit to take away and the `kits.ron` to put back."));
        };
        let dir = Self::dir(&root, name);
        let (library, policy) = new_kit_files(name);
        let expected = [
            (dir.join(bevy_grimoire_core::policy::LIBRARY_FILE), library),
            (dir.join(bevy_grimoire_core::policy::POLICY_FILE), policy),
        ];
        let listed: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| Refusal::new(format!("`{name}` has no directory to take away: {e}")))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        let untouched = listed.len() == expected.len()
            && expected.iter().all(|(path, text)| std::fs::read_to_string(path).is_ok_and(|now| now == *text));
        if !untouched {
            return Err(Refusal::new(format!(
                "`{name}` holds more than the two files `kit_create` wrote, or one of them has been \
                 edited, so it is not taken away. Nothing was changed."
            )));
        }
        if write {
            for (path, _) in &expected {
                std::fs::remove_file(path)
                    .map_err(|e| Refusal::new(format!("could not remove {}: {e}", path.display())))?;
            }
            std::fs::remove_dir(&dir)
                .map_err(|e| Refusal::new(format!("could not remove {}: {e}", dir.display())))?;
            bevy_grimoire_core::ron_surgery::save_atomic(&kits_path(&root), prior).map_err(Refusal::new)?;
        }
        Ok(Outcome {
            summary: format!("took the empty kit `{name}` away and put kits.ron back"),
            affected: 1,
            saved: write,
            inverse: Some(json!({"name": name})),
            after: Some(json!({"kit": name, "kits": prior})),
        })
    }

    fn plan_create(ctx: &Ctx, p: &CreateParams) -> Result<(PathBuf, String), Refusal> {
        let root = root(ctx)?;
        let name = bevy_grimoire_core::naming::to_snake_case(&p.name);
        if name.is_empty() {
            return Err(Refusal::new(format!(
                "`{}` leaves nothing usable as a name. Kits are snake_case — lowercase letters, digits and \
                 single underscores, starting with a letter.",
                p.name
            )));
        }
        if Self::dir(&root, &name).exists() {
            return Err(Refusal::new(format!(
                "`{name}` already exists under {GRIMOIRE_DIR}. Pick another name, or open the one that is there."
            )));
        }
        // **Before a directory is made**, so a folder that is not a project is refused without
        // leaving an empty kit behind it.
        kits_text(&root)?;
        Ok((root, name))
    }
}

impl Operation for KitCreate {
    fn name(&self) -> &'static str {
        "kit_create"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["kits", "library", "policy"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        CreateParams::parse(params).is_ok_and(|p| p.remove.is_some() || p.kits.is_some())
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["name"],
            "properties": {
                "name": {"type": "string", "description": "What the kit is called. Forced to snake_case: lowercase letters, digits and single underscores, starting with a letter. A kit is a directory under assets/grimoire with an empty library.ron and project.ron, bound in kits.ron as its own namespace; the first kit of a project is also where new work lands."}
            }
        })
    }
    /// **The kit's directory and `kits.ron` as an undo finds them**, against what the change recorded.
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let name = after.get("kit")?.as_str()?;
        let root = ctx.root_or_project()?;
        Some(json!({"kit": name, "kits": kits_text(&root).ok()?}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = CreateParams::parse(params)?;
        if p.remove.is_some() || p.kits.is_some() {
            let out = Self::removal(ctx, &p, false)?;
            return Ok(out.after.unwrap_or_default());
        }
        let (_, name) = Self::plan_create(ctx, &p)?;
        Ok(json!({"kit": name}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = CreateParams::parse(params)?;
        if p.remove.is_some() || p.kits.is_some() {
            return Self::removal(ctx, &p, true);
        }
        let (root, name) = Self::plan_create(ctx, &p)?;
        let prior = kits_text(&root)?;
        let dir = crate::chooser::create_kit(&root, &name).map_err(Refusal::new)?;
        let after = json!({"kit": name, "kits": kits_text(&root)?, "path": dir.to_string_lossy()});
        Ok(Outcome {
            summary: format!("made the empty kit `{name}` and bound it"),
            affected: 1,
            saved: true,
            inverse: Some(json!({"remove": name, "kits": prior})),
            after: Some(after),
        })
    }
}

// ── kit_set_authoring ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringParams {
    /// The bound kit's directory, as the chooser's kit column names it. Absent on an undo's put-back
    /// to "no authoring kit".
    #[serde(default)]
    pub kit: Option<String>,
    /// **Undo only.** Point `authoring` at no kit again (the state before the first was chosen).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub none: bool,
}

impl AuthoringParams {
    fn parse(raw: &serde_json::Value) -> Result<AuthoringParams, Refusal> {
        let p: AuthoringParams = serde_json::from_value(raw.clone())
            .map_err(|e| Refusal::new(format!("this is not a kit to author in: {e}. The only field is `kit`.")))?;
        match (&p.kit, p.none) {
            (Some(_), false) | (None, true) => Ok(p),
            (Some(_), true) => Err(Refusal::new("`none` is an undo's form and takes no `kit`.")),
            (None, false) => Err(Refusal::new("name the kit new work should land in: `kit`.")),
        }
    }
}

fn authoring_of(root: &Path) -> Result<Option<String>, Refusal> {
    let text = kits_text(root)?;
    bevy_grimoire_core::kits::Kits::parse(&text)
        .map(|k| k.authoring)
        .map_err(|e| Refusal::new(format!("{}: {e}", kits_path(root).display())))
}

/// **Point `authoring` at a bound kit**: where an imported mesh and a new tile land when the command
/// line does not say.
pub struct KitSetAuthoring;

impl Operation for KitSetAuthoring {
    fn name(&self) -> &'static str {
        "kit_set_authoring"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["kits"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        AuthoringParams::parse(params).is_ok_and(|p| p.none)
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["kit"],
            "properties": {
                "kit": {"type": "string", "description": "The directory of a kit this project binds (see grimoire/state or the kits column). New imports and tiles land in it. A kit that is not bound is refused, naming the ones that are, and the kit that is already authoring is refused as a change of nothing."}
            }
        })
    }
    fn state_of(&self, ctx: &Ctx, _after: &serde_json::Value) -> Option<serde_json::Value> {
        Some(json!({"authoring": authoring_of(&ctx.root_or_project()?).ok()?}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = AuthoringParams::parse(params)?;
        let root = root(ctx)?;
        let now = authoring_of(&root)?;
        if p.none {
            return Ok(json!({"authoring": null}));
        }
        let kit = p.kit.unwrap_or_default();
        // **A kit the project does not bind is refused here as the write refuses it**, so a preview
        // does not promise what the commit will not do.
        let kits = bevy_grimoire_core::kits::Kits::parse(&kits_text(&root)?).map_err(Refusal::new)?;
        if !kits.bind.iter().any(|b| b.dir == kit) {
            return Err(Refusal::new(format!(
                "`{kit}` is not bound in {}. The bound kits are: {}.",
                kits_path(&root).display(),
                kits.bind.iter().map(|b| format!("`{}`", b.dir)).collect::<Vec<_>>().join(", ")
            )));
        }
        if now.as_deref() == Some(kit.as_str()) {
            return Err(Refusal::new(format!("`{kit}` is already where new work lands, so nothing was changed.")));
        }
        Ok(json!({"authoring": kit}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = AuthoringParams::parse(params)?;
        let root = root(ctx)?;
        let before = authoring_of(&root)?;
        let (summary, after) = if p.none {
            let text = kits_text(&root)?;
            let mut kits = bevy_grimoire_core::kits::Kits::parse(&text).map_err(Refusal::new)?;
            kits.authoring = None;
            let out = kits.to_ron().map_err(Refusal::new)?;
            bevy_grimoire_core::kits::Kits::parse(&out).map_err(Refusal::new)?;
            bevy_grimoire_core::ron_surgery::save_atomic(&kits_path(&root), &out).map_err(Refusal::new)?;
            ("new work lands in no particular kit again".to_owned(), None)
        } else {
            let kit = self.plan(ctx, params)?["authoring"].as_str().unwrap_or_default().to_owned();
            crate::chooser::set_authoring(&root, &kit).map_err(Refusal::new)?;
            (format!("new work lands in `{kit}`"), Some(kit))
        };
        let inverse = match &before {
            Some(kit) => json!({"kit": kit}),
            None => json!({"none": true}),
        };
        Ok(Outcome {
            summary,
            affected: 1,
            saved: true,
            inverse: Some(inverse),
            after: Some(json!({"authoring": after})),
        })
    }
}
