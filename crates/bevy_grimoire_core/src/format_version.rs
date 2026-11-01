//! **The version a saved format carries, and what reading a file of another one means** (#202, H5 step 4).
//!
//! A format with no version number cannot change shape without breaking the projects already on disk:
//! a reader cannot tell an old file from a damaged one, and a file written by a newer build is read as
//! if it were this build's. So a format's root struct has a `version` field, declared with
//!
//! ```ignore
//! #[serde(default = "bevy_grimoire_core::format_version::first", deserialize_with = "bevy_grimoire_core::format_version::read::<_, MY_VERSION>")]
//! pub version: u32,
//! ```
//!
//! - `default = first` is for the files an earlier release wrote, which have no `version`: they are
//!   the first version of the format, whatever version the build reads today, so the file reads (and a
//!   later version's migration starts from it) and nothing on disk has to be rewritten.
//! - `deserialize_with = read` is the refusal. It sits in the type's own deserialiser, not in a
//!   `parse` function beside it, because a `parse` only protects the callers that use it: an editor
//!   operation that reads the file with a plain `ron::from_str` and writes it back would otherwise
//!   write a newer file at this build's version (the failure `destructible.ron` had until slice 1 of
//!   #202). With the refusal in the type, every reader of the type refuses, and the refusal names the
//!   field (`version`) like any other.
//!
//! A format whose reader has to take more than one version (a migration) does not use [`read`]: it
//! reads the version first and decides. None does yet.

use serde::{Deserialize, Deserializer, de::Error};

/// The version of a format that had none when it was versioned: every file written before the
/// version existed is this one.
pub const FIRST: u32 = 1;

/// [`FIRST`], for `#[serde(default = "...")]`.
pub fn first() -> u32 {
    FIRST
}

/// **Read a `version` field that must be `V`**: any other number is refused, saying which version this
/// build reads, so an author knows a file from a newer release is not damaged.
pub fn read<'de, D: Deserializer<'de>, const V: u32>(deserializer: D) -> Result<u32, D::Error> {
    let found = u32::deserialize(deserializer)?;
    if found == V {
        Ok(found)
    } else {
        Err(D::Error::custom(format!("version {found} is not the {V} this build reads")))
    }
}
