//! **The installed macOS app updates itself from this repository's GitHub Releases.**
//!
//! A release tagged `bevy_cauldron-vX.Y.Z` carries `Cauldron-X.Y.Z-macos-arm64.dmg`, the same app as a
//! zip, and `SHA256SUMS` (`tools/release/macos/build_app.sh`, `.github/workflows/release-macos.yml`). At
//! launch the app asks the releases feed for a newer tag ([`newest`]); the launcher always shows the
//! button, and the editor's chrome bar shows it only once there is something to say ([`Updater::in_chrome`]).
//! One click downloads the zip and checks it against the release's sums ([`fetch_verified`]), then checks
//! its signature, its team against the running app's, and Gatekeeper's verdict. A small shell script
//! ([`SWAP_SH`], in its own process group) waits for this process to exit, **for [`SWAP_KILL_AFTER_S`] seconds at
//! most and then ends it** (#337), swaps the bundle, and opens the new one with the same arguments.
//!
//! **Only `main` inserts the [`Updater`], and only inside a `.app`** ([`bundle_of`]): `cargo run` and every
//! test editor have none, so a development build never replaces itself.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Mutex;
use std::time::Duration;

use bevy::prelude::*;
use bevy_gooey::{button, observe};
use sha2::{Digest, Sha256};

/// The releases this app looks for a newer version among, newest first.
pub const FEED: &str = "https://api.github.com/repos/bevy-coven/bevy_grimoire/releases?per_page=30";

/// The tag of a Cauldron release, before its version: tags of the workspace's other crates share the feed.
pub const TAG_PREFIX: &str = "bevy_cauldron-v";

/// The asset holding `shasum -a 256` lines for the release's DMG and zip.
pub const SUMS: &str = "SHA256SUMS";

/// Names a feed to read instead of [`FEED`]: a local server, for the update smoke.
pub const FEED_ENV: &str = "CAULDRON_UPDATE_FEED";

/// The app's folder name inside the update zip (`ditto -c -k --keepParent dist/Cauldron.app`).
pub const APP: &str = "Cauldron.app";

/// How long a connection may take to open. There is no limit on the whole transfer: the zip is ~100 MB.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// How much of the zip is read, hashed and written at a time.
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;

/// The refusal while a map has unsaved edits.
pub const SAVE_FIRST: &str = "Save the map first: updating restarts Cauldron";

/// The zip a release carries for `v`. The DMG is the same name with `.dmg`.
pub fn zip_name(v: &semver::Version) -> String {
    format!("Cauldron-{v}-macos-arm64.zip")
}

/// A newer release, and where its zip and its sums are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub version: semver::Version,
    pub zip_url: String,
    pub sums_url: String,
}

/// **The newest published Cauldron release above `current`**, from the GitHub releases JSON.
///
/// A release counts when its tag is [`TAG_PREFIX`] and a semver version with no pre-release part, it is
/// neither a draft nor a pre-release, and it carries both its [`zip_name`] and [`SUMS`]. `Err` when the
/// text is not a JSON array.
pub fn newest(releases_json: &str, current: &semver::Version) -> Result<Option<Offer>, String> {
    use serde_json::Value;
    let value: Value = serde_json::from_str(releases_json).map_err(|e| format!("the releases feed is not JSON: {e}"))?;
    let releases = value.as_array().ok_or("the releases feed is not a list of releases")?;
    let mut best: Option<Offer> = None;
    for release in releases {
        let Some(rest) = release.get("tag_name").and_then(Value::as_str).and_then(|t| t.strip_prefix(TAG_PREFIX)) else {
            continue;
        };
        let published = release.get("draft").and_then(Value::as_bool) == Some(false)
            && release.get("prerelease").and_then(Value::as_bool) == Some(false);
        let Ok(version) = semver::Version::parse(rest) else { continue };
        if !published || !version.pre.is_empty() || version <= *current {
            continue;
        }
        let assets = release.get("assets").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
        let url_of = |name: &str| {
            assets
                .iter()
                .find(|a| a.get("name").and_then(Value::as_str) == Some(name))
                .and_then(|a| a.get("browser_download_url").and_then(Value::as_str))
                .map(str::to_owned)
        };
        let (Some(zip_url), Some(sums_url)) = (url_of(&zip_name(&version)), url_of(SUMS)) else { continue };
        if best.as_ref().is_none_or(|b| version > b.version) {
            best = Some(Offer { version, zip_url, sums_url });
        }
    }
    Ok(best)
}

/// The lowercase sha256 `sums` gives for `file`, from `shasum -a 256` lines (`<hex>  <name>`, or
/// `<hex> *<name>` in binary mode).
pub fn expected_sum(sums: &str, file: &str) -> Result<String, String> {
    const SHA256_HEX_CHARS: usize = 64;
    for line in sums.lines() {
        let Some((hex, name)) = line.split_once(' ') else { continue };
        let name = name.trim_start();
        if name.strip_prefix('*').unwrap_or(name) != file {
            continue;
        }
        if hex.len() != SHA256_HEX_CHARS || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("{SUMS} has no sha256 for {file}: `{hex}`"));
        }
        return Ok(hex.to_ascii_lowercase());
    }
    Err(format!("{SUMS} has no line for {file}"))
}

/// The HTTP client for the feed and the downloads: a bounded connect, no bound on the transfer.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_connect(Some(CONNECT_TIMEOUT)).build().into()
}

fn get(agent: &ureq::Agent, url: &str) -> Result<ureq::http::Response<ureq::Body>, String> {
    agent
        .get(url)
        .header("User-Agent", &format!("Cauldron/{}", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("{url}: {e}"))
}

fn get_text(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    get(agent, url)?.body_mut().read_to_string().map_err(|e| format!("{url}: {e}"))
}

/// **Downloads `offer`'s zip into `dir` and proves it is the one the release names.** The sums come
/// first, so a release with no line for its zip is refused before ~100 MB are fetched. A zip whose
/// sha256 differs is deleted and refused.
pub fn fetch_verified(offer: &Offer, dir: &Path) -> Result<PathBuf, String> {
    fetch_verified_reporting(offer, dir, &|_, _| {})
}

/// [`fetch_verified`], saying how the download is going: `progress(received, total)` as the bytes
/// arrive. The checksum is still proved before the file is kept, whatever the progress said.
pub fn fetch_verified_reporting(
    offer: &Offer,
    dir: &Path,
    progress: &dyn Fn(u64, Option<u64>),
) -> Result<PathBuf, String> {
    let agent = agent();
    let name = zip_name(&offer.version);
    let want = expected_sum(&get_text(&agent, &offer.sums_url)?, &name)?;
    let path = dir.join(&name);
    let got = stream(&agent, &offer.zip_url, &path, progress).inspect_err(|_| {
        let _ = std::fs::remove_file(&path);
    })?;
    if got != want {
        let _ = std::fs::remove_file(&path);
        return Err(format!("checksum mismatch for {name}: expected {want}, got {got}"));
    }
    Ok(path)
}

/// Streams `url` into `to`, calling `progress(received, total)` as it goes.
///
/// **`progress` is what makes a download bar possible** (`feedback-1793903311065`): the response's own
/// `Content-Length`, and the bytes written so far. `total` is `None` when the server states no length,
/// which is a bar that fills nothing rather than a bar that lies.
fn stream(
    agent: &ureq::Agent,
    url: &str,
    to: &Path,
    progress: &dyn Fn(u64, Option<u64>),
) -> Result<String, String> {
    let response = get(agent, url)?;
    let total = response.body().content_length();
    let mut reader = response.into_body().into_reader();
    let mut file = std::fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; DOWNLOAD_CHUNK_BYTES];
    let mut received = 0u64;
    progress(0, total);
    loop {
        let n = reader.read(&mut chunk).map_err(|e| format!("{url}: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
        file.write_all(&chunk[..n]).map_err(|e| format!("{}: {e}", to.display()))?;
        received += n as u64;
        progress(received, total);
    }
    file.flush().map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Where the running executable sits, as far as updating it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bundle {
    /// A `.app` in a folder it can be replaced in.
    Installed(PathBuf),
    /// A `.app` run from the disk image (`/Volumes/…`) or from Gatekeeper's read-only copy of a
    /// quarantined download (`…/AppTranslocation/…`): replacing it would replace nothing a person keeps.
    Translocated(PathBuf),
    /// Not inside a `.app`: `cargo run`, a test, a bare binary.
    NotABundle,
}

/// Which [`Bundle`] `exe` is in: `<x>.app/Contents/MacOS/<exe>`.
pub fn bundle_of(exe: &Path) -> Bundle {
    let macos = exe.parent();
    let contents = macos.and_then(Path::parent);
    let app = contents.and_then(Path::parent);
    let (Some(macos), Some(contents), Some(app)) = (macos, contents, app) else { return Bundle::NotABundle };
    let is_bundle = macos.file_name().is_some_and(|n| n == "MacOS")
        && contents.file_name().is_some_and(|n| n == "Contents")
        && app.extension().is_some_and(|e| e == "app");
    if !is_bundle {
        return Bundle::NotABundle;
    }
    if app.to_string_lossy().contains("/AppTranslocation/") || app.starts_with("/Volumes") {
        Bundle::Translocated(app.to_path_buf())
    } else {
        Bundle::Installed(app.to_path_buf())
    }
}

/// Where an update is in its life.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(Offer),
    /// **Downloading, with the bytes that have arrived** and the ones the response said it would send
    /// (`None` when the server stated no length). The pair is what a progress bar draws, and the note
    /// that asked for one is `feedback-1793903311065`.
    Downloading { version: semver::Version, received: u64, total: Option<u64> },
    Installing(semver::Version),
    Failed(String),
}

/// What a background job answers.
enum Msg {
    Checked(Result<Option<Offer>, String>),
    Staged(Result<PathBuf, String>),
    /// **How much of the download has arrived**, sent on its own channel so it does not consume the
    /// job's one answer: [`Updater::poll`] returns early while the answer channel is empty, which is
    /// every frame of a download.
    Progress { received: u64, total: Option<u64> },
}

/// **The running app's own update.** Inserted by `main` only, inside a `.app`.
#[derive(Resource)]
pub struct Updater {
    /// The releases JSON to read: [`FEED`], or [`FEED_ENV`]'s value.
    pub feed: String,
    /// The `.app` this process runs from, which an update replaces.
    pub bundle: PathBuf,
    /// Why this app cannot update where it is, said instead of starting.
    pub blocked: Option<String>,
    pub state: UpdateState,
    /// **Did a person press the button for the job in flight?**
    ///
    /// The launch check runs on its own ([`start_the_first_check`]). It used to answer into the
    /// launcher's problem line exactly as a pressed one does, so every launch opened with a sticky
    /// *"Cauldron v0.2.4 is the newest release"* in the danger skin — measured in the shipped 0.2.4
    /// screenshot on 2026-11-05, and it was the first thing on a first-run screen. **A check nobody
    /// asked for has nothing to report when it finds nothing**: the button's own label already says
    /// `Up to date — v…`. An update that *is* available, and a failure, speak either way.
    manual: bool,
    /// The offer being downloaded, so a refusal at the last step can offer it again.
    staging: Option<Offer>,
    /// The answer of the job in flight, if one is.
    rx: Option<Mutex<mpsc::Receiver<Msg>>>,
    /// **How far the download has got, on its own channel.** The answer channel is read only when it
    /// has something, and a download's whole point is the frames before it does — so progress is
    /// kept apart from the one answer a job sends.
    progress_rx: Option<Mutex<mpsc::Receiver<Msg>>>,
}

impl Updater {
    pub fn new(feed: String, bundle: PathBuf) -> Self {
        Self {
            feed,
            bundle,
            blocked: None,
            state: UpdateState::Idle,
            manual: false,
            staging: None,
            rx: None,
            progress_rx: None,
        }
    }

    /// **What the button reads, including the answer.**
    ///
    /// `UpToDate` used to read the same words as `Idle`, and the cost of that was measured at the
    /// keyboard on 2026-11-05: a person pressed *Check for updates*, the check finished in half a
    /// second, the label went back to *Check for updates*, and the button looked like it had not
    /// worked. A check that finds nothing is an answer, and an answer with no words is
    /// indistinguishable from a press that was dropped — so it says which version is the newest, and
    /// the person can see that it *is* the one they have.
    pub fn label(&self) -> String {
        match &self.state {
            UpdateState::Idle => "Check for updates".to_owned(),
            UpdateState::UpToDate => {
                format!("Up to date — v{}", env!("CARGO_PKG_VERSION"))
            }
            UpdateState::Checking => "Checking for updates…".to_owned(),
            UpdateState::Available(offer) => format!("Update to v{}", offer.version),
            UpdateState::Downloading { version, received, total } => match total {
                // **A number a person can watch**, which is what the note asked for: "a download bar
                // whenever you hit Update" (`feedback-1793903311065`). Whole percent, because a
                // tenth of a percent on 100 MB is a flicker rather than information.
                Some(total) if *total > 0 => {
                    format!("Downloading v{version}… {}%", (received.saturating_mul(100) / total).min(100))
                }
                _ => format!("Downloading v{version}…"),
            },
            UpdateState::Installing(v) => format!("Restarting into v{v}…"),
            UpdateState::Failed(_) => "Update failed — try again".to_owned(),
        }
    }

    /// **Is there anything for a person to do about updating?**
    ///
    /// The home's sidebar used to show [`Self::label`] as a button always, so on a first run it read
    /// `Up to date — v0.2.4` in the same skin as `Projects`, `Learn` and `Settings` — a status readout
    /// dressed as a page tab, inviting a press that does nothing, and printing the version a second
    /// time two lines under `Version 0.2.4` (design review, 2026-11-05). The sidebar now shows a quiet
    /// version line and **this button only when it is true**: there is an update, one is downloading,
    /// or one failed.
    pub fn actionable(&self) -> bool {
        matches!(
            self.state,
            UpdateState::Available(_) | UpdateState::Downloading { .. } | UpdateState::Installing(_) | UpdateState::Failed(_)
        )
    }

    /// Whether the editor's chrome bar shows the button: only when there is an update to take, one in
    /// progress, or one that failed. Nothing to offer leaves the bar as it was.
    ///
    /// **`UpToDate` is deliberately not here.** An up-to-date answer is not something to act on, and
    /// the bar is the editor's working surface — a button that says "nothing to do" would sit over a
    /// map being authored. On the home the answer is one quiet line, and a button only when
    /// [`Self::actionable`].
    pub fn in_chrome(&self) -> bool {
        self.actionable()
    }

    /// Start a check. `manual` says a person pressed for it, which is what decides whether an
    /// answer of "nothing newer" is worth saying — see [`Self::manual`].
    fn begin_check(&mut self, manual: bool) {
        self.state = UpdateState::Checking;
        self.manual = manual;
        let feed = self.feed.clone();
        self.spawn(move || Msg::Checked(check(&feed)));
    }

    fn begin_download(&mut self, offer: Offer) {
        self.state = UpdateState::Downloading { version: offer.version.clone(), received: 0, total: None };
        self.staging = Some(offer.clone());
        let running = self.bundle.clone();
        // **The progress channel is made here and read by `progress()`**, so the frames of a download
        // carry the bytes that arrived without touching the channel the answer will come on.
        let (send, receive) = mpsc::channel::<Msg>();
        self.progress_rx = Some(Mutex::new(receive));
        let (answer_send, answer) = mpsc::channel();
        self.rx = Some(Mutex::new(answer));
        std::thread::spawn(move || {
            let report = |received: u64, total: Option<u64>| {
                let _ = send.send(Msg::Progress { received, total });
            };
            let _ = answer_send.send(Msg::Staged(stage(&offer, &running, &report)));
        });
    }

    /// **The latest progress a download has reported**, as `(received, total)`, taken from the channel
    /// and written into the state. Called every frame by [`progress`]: a `Some` return means the state
    /// changed, which is what makes the bar move.
    fn take_progress(&mut self) -> bool {
        let Some(rx) = self.progress_rx.as_ref() else { return false };
        let Ok(rx) = rx.lock() else { return false };
        let mut moved = false;
        // **Drained, not read once**: a frame can fall behind several 64 KiB chunks, and a bar that
        // showed every third chunk would crawl.
        while let Ok(Msg::Progress { received, total }) = rx.try_recv() {
            if let UpdateState::Downloading { received: had, total: had_total, .. } = &mut self.state
                && (*had != received || *had_total != total)
            {
                *had = received;
                *had_total = total;
                moved = true;
            }
        }
        moved
    }

    fn spawn(&mut self, job: impl FnOnce() -> Msg + Send + 'static) {
        let (send, receive) = mpsc::channel();
        self.rx = Some(Mutex::new(receive));
        std::thread::spawn(move || {
            // An answer nobody reads (the app closed) is dropped with the channel.
            let _ = send.send(job());
        });
    }
}

/// Reads the feed and picks [`newest`] above this build's version.
fn check(feed: &str) -> Result<Option<Offer>, String> {
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| format!("this build's version: {e}"))?;
    newest(&get_text(&agent(), feed)?, &current)
}

/// `~/Library/Caches/Cauldron/updates/<version>`: the download, the unpacked app and the swap script.
#[cfg(target_os = "macos")]
fn update_dir(version: &semver::Version) -> Result<PathBuf, String> {
    let cache = dirs::cache_dir().ok_or("this user has no cache folder")?;
    Ok(cache.join("Cauldron").join("updates").join(version.to_string()))
}

/// **Downloads, unpacks and checks the new app**, and proves the running one can be replaced. The new
/// app's path is the answer; nothing outside the update folder has been touched.
#[cfg(target_os = "macos")]
fn stage(offer: &Offer, running: &Path, progress: &dyn Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    use std::process::Command;
    let ran = |program: &str, args: &[&std::ffi::OsStr]| {
        Command::new(program).args(args).output().map_err(|e| format!("{program}: {e}"))
    };
    let dir = update_dir(&offer.version)?;
    // This folder is the updater's own: an earlier attempt's leftovers are not kept.
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let zip = fetch_verified_reporting(offer, &dir, progress)?;
    let unpacked = dir.join("unpacked");
    // `ditto`, not `unzip`: it keeps the code signature and the stapled ticket intact.
    let out = ran("/usr/bin/ditto", &["-x".as_ref(), "-k".as_ref(), zip.as_os_str(), unpacked.as_os_str()])?;
    if !out.status.success() {
        return Err(format!("the download would not unpack: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let new = unpacked.join(APP);
    if !new.is_dir() {
        return Err(format!("the download has no {APP}"));
    }
    let verify = ran("/usr/bin/codesign", &["--verify".as_ref(), "--deep".as_ref(), "--strict".as_ref(), new.as_os_str()])?;
    if !verify.status.success() {
        return Err("the downloaded app's signature does not verify".to_owned());
    }
    let team = |app: &Path| -> Result<Option<String>, String> {
        let out = ran("/usr/bin/codesign", &["-dv".as_ref(), "--verbose=2".as_ref(), app.as_os_str()])?;
        Ok(team_of(&String::from_utf8_lossy(&out.stderr)))
    };
    let ours = team(running)?;
    if team(&new)? != ours {
        return Err("the download is signed by another team".to_owned());
    }
    // A Developer ID build must also be notarized; an ad-hoc build (no team) has nothing to assess.
    if ours.is_some() {
        let assess = ran("/usr/sbin/spctl", &["--assess".as_ref(), "--type".as_ref(), "execute".as_ref(), new.as_os_str()])?;
        if !assess.status.success() {
            return Err(format!("Gatekeeper refuses the download: {}", String::from_utf8_lossy(&assess.stderr).trim()));
        }
    }
    let parent = running.parent().ok_or_else(|| format!("{} has no folder", running.display()))?;
    let probe = parent.join(".cauldron-update-probe");
    bevy_grimoire_core::atomic_file::write_cache(&probe, b"").and_then(|()| std::fs::remove_file(&probe)).map_err(|_| {
        format!("{} is not writable: install Cauldron in a folder you own, or download the DMG", parent.display())
    })?;
    Ok(new)
}

#[cfg(not(target_os = "macos"))]
fn stage(_offer: &Offer, _running: &Path, _progress: &dyn Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    Err("updates are macOS-only".to_owned())
}

/// The `TeamIdentifier=` of `codesign -dv` output; `None` when absent or `not set` (an ad-hoc signature).
#[cfg(target_os = "macos")]
fn team_of(codesign_stderr: &str) -> Option<String> {
    codesign_stderr
        .lines()
        .find_map(|l| l.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "not set")
        .map(str::to_owned)
}

/// **Waits for the old process, but only so long**, moves the old app aside, puts the new one in its
/// place and opens it with the old arguments. Any failed move puts the old app back and opens that
/// instead.
///
/// Arguments: the old app's pid, the old and the new bundle, the program that relaunches, how many
/// seconds the old process has before it is asked to end (`TERM`) and before it is made to (`KILL`),
/// then the arguments the old app was started with. **An old process that has not left by then is
/// ended**, because it was asked to quit for this update and a swap that waits for ever on a window-less
/// process is the freeze the person met (#337): the app stayed on its old version until they killed it
/// and started it by hand. A pid that now names another program, or a zombie, counts as gone.
pub const SWAP_SH: &str = r#"#!/bin/sh
exec >"$(dirname "$0")/swap.log" 2>&1
pid="$1"; old="$2"; new="$3"; open="$4"; term_after="$5"; kill_after="$6"; shift 6
relaunch() { if [ "$#" -gt 0 ]; then "$open" "$old" --args "$@"; else "$open" "$old"; fi; }
comm="$(ps -p "$pid" -o comm= 2>/dev/null)"
alive() {
  state="$(ps -p "$pid" -o stat= 2>/dev/null | tr -d ' ')"
  [ -n "$state" ] && [ "${state#Z}" = "$state" ] && [ "$(ps -p "$pid" -o comm= 2>/dev/null)" = "$comm" ]
}
waited=0
while alive; do
  sleep 0.2
  waited=$((waited + 1))
  if [ "$waited" -eq $((term_after * 5)) ]; then kill -TERM "$pid" 2>/dev/null; fi
  if [ "$waited" -ge $((kill_after * 5)) ]; then kill -KILL "$pid" 2>/dev/null; fi
done
backup="$old.previous"
rm -rf "$backup"
if ! mv "$old" "$backup"; then relaunch "$@"; exit 1; fi
if mv "$new" "$old"; then rm -rf "$backup"; relaunch "$@"; else mv "$backup" "$old"; relaunch "$@"; exit 1; fi
"#;

/// The program that opens the new app, and the old one when the swap fails.
pub const RELAUNCH_PROGRAM: &str = "open";

/// Seconds the old process has to leave on its own before the swap asks it to end.
pub const SWAP_TERM_AFTER_S: u32 = 10;

/// Seconds after which the swap ends the old process whether it was asked or not.
pub const SWAP_KILL_AFTER_S: u32 = 15;

/// One run of [`SWAP_SH`]: which process to wait for, which bundle replaces which, and what to do if
/// the process does not leave.
pub struct Swap<'a> {
    /// The old app's process id.
    pub pid: u32,
    /// The bundle being replaced.
    pub running: &'a Path,
    /// The bundle replacing it.
    pub new: &'a Path,
    /// The program that relaunches the app ([`RELAUNCH_PROGRAM`]).
    pub relaunch: &'a str,
    /// Seconds before the old process is asked to end ([`SWAP_TERM_AFTER_S`]).
    pub term_after_s: u32,
    /// Seconds before it is made to ([`SWAP_KILL_AFTER_S`]).
    pub kill_after_s: u32,
    /// The arguments the old app was started with, passed on to the new one.
    pub args: Vec<std::ffi::OsString>,
}

#[cfg(unix)]
impl Swap<'_> {
    /// Writes `script` and starts it, not waiting for it. **It runs in its own process group**, so
    /// whatever ends the old app by group (a person force-quitting a window-less app, the terminal's
    /// job control) cannot end the swap with it: the script outlives the process it is waiting for.
    pub fn start(&self, script: &Path) -> Result<std::process::Child, String> {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        bevy_grimoire_core::atomic_file::write_cache(script, SWAP_SH).map_err(|e| format!("{}: {e}", script.display()))?;
        Command::new("/bin/sh")
            .arg(script)
            .arg(self.pid.to_string())
            .arg(self.running)
            .arg(self.new)
            .arg(self.relaunch)
            .arg(self.term_after_s.to_string())
            .arg(self.kill_after_s.to_string())
            .args(&self.args)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("the swap did not start: {e}"))
    }
}

/// Starts [`SWAP_SH`] for `new` over `running`, not waiting for it: it waits for this process to exit.
#[cfg(target_os = "macos")]
fn swap(new: &Path, running: &Path) -> Result<(), String> {
    let dir = new
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("{} is not in an update folder", new.display()))?;
    Swap {
        pid: std::process::id(),
        running,
        new,
        relaunch: RELAUNCH_PROGRAM,
        term_after_s: SWAP_TERM_AFTER_S,
        kill_after_s: SWAP_KILL_AFTER_S,
        args: std::env::args_os().skip(1).collect(),
    }
    .start(&dir.join("swap.sh"))
    .map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn swap(_new: &Path, _running: &Path) -> Result<(), String> {
    Err("updates are macOS-only".to_owned())
}

/// The update button, wherever it is shown.
#[derive(Component)]
pub struct UpdateButton;

/// A button reading `label` that checks, offers or installs an update.
pub fn update_button(label: String) -> impl bevy::prelude::Bundle {
    // `Pickable` narrows the chrome bar's `Pickable::IGNORE` for the one node on it that is a button.
    (button(label), UpdateButton, bevy::picking::Pickable::default(), observe(on_activate))
}

/// **The download bar**, shown beside the update button while an update is coming down
/// (`feedback-1793903311065`: *"Can we make this a download bar whenever you hit Update, then when the
/// update is finished, switch back to a button"*).
///
/// A gooey [`bevy_gooey::Meter`] rather than a bar of this crate's own: it eases its fill, and its
/// fraction is clamped and never NaN, so a server that states no length leaves it empty rather than
/// filling it wrongly. Its width is a share of the sidebar's content width, because the bar stands
/// under the version line.
#[derive(Component)]
pub struct UpdateBar;

/// How wide the download bar is, in logical pixels: the sidebar's content width.
pub const UPDATE_BAR_W: f32 = 216.0;
/// How tall it is, in logical pixels: two pixels of air above the button's height.
pub const UPDATE_BAR_H: f32 = 6.0;

/// The bar for `received` of `total`, or nothing when there is no download to show.
fn bar_for(received: u64, total: Option<u64>) -> impl bevy::prelude::Bundle {
    let (value, max) = bar_reading(received, total);
    // **`meter` already carries its own `Node`**, so the size goes beside the meter rather than in a
    // second one: two `Node`s in one bundle is a duplicate-component panic, not a bigger bar.
    (bevy_gooey::meter(value, max), UpdateBar, UpdateBarSize)
}

/// **What the bar's meter says for a download** — the one place that arithmetic lives, so a bar that is
/// spawned and a bar that is updated cannot disagree.
///
/// **With no stated length it says nothing about progress**, rather than carrying a byte count against
/// a maximum of zero: `meter_fraction` would draw it empty either way, but a value meaning "4,000 of
/// nothing" is a number a later reader would trust.
fn bar_reading(received: u64, total: Option<u64>) -> (f32, f32) {
    let total = total.unwrap_or(0);
    let value = if total == 0 { 0.0 } else { received.min(total) as f32 };
    (value, total as f32)
}

/// **A bar whose node has not been given its size yet.** `meter` spawns the `Node`, so the size is set
/// on the bar's own entity rather than in a second one; this marks the ones still waiting.
#[derive(Component)]
struct UpdateBarSize;

/// **Keeps the download bar in step with the updater**: beside the button while an update is coming
/// down, gone the moment it is not.
///
/// A bar that stayed after the download finished would be the "switch back to a button" the note asks
/// for, undone. So the bar exists only while [`Updater::state`] is `Downloading`, and its value is the
/// bytes that have arrived.
fn show_the_bar(
    mut commands: Commands,
    updater: Option<Res<Updater>>,
    bars: Query<(Entity, &bevy_gooey::Meter, &ChildOf), With<UpdateBar>>,
    buttons: Query<&ChildOf, With<UpdateButton>>,
) {
    let want = match updater.as_deref().map(|u| &u.state) {
        Some(UpdateState::Downloading { received, total, .. }) => Some((*received, *total)),
        _ => None,
    };
    // **The bar's node is `meter`'s own**, so its size is set on the same entity: a second `Node` in the
    // bundle is a duplicate component, and a wrapper would put the meter's fill inside a node the meter
    // does not measure. `size_the_bar` does it once, when the bar is spawned.
    let placed: Vec<(Entity, f32, f32, Entity)> = bars
        .iter()
        .map(|(entity, meter, parent)| (entity, meter.value, meter.max, parent.parent()))
        .collect();
    match want {
        Some((received, total)) => {
            // **The same arithmetic as the spawn**, so a bar that already exists and a bar that is new
            // cannot disagree about what "4,000 bytes of no stated length" means.
            let (value, max) = bar_reading(received, total);
            if placed.is_empty() {
                // **Under the button**, in the same column, so the bar is where the person is looking.
                let Some(parent) = buttons.iter().next().map(|p| p.parent()) else { return };
                commands.entity(parent).with_children(|column| {
                    column.spawn(bar_for(received, total));
                });
            } else {
                for (entity, had, had_max, _) in placed {
                    if had != value || had_max != max {
                        commands.entity(entity).insert(bevy_gooey::Meter { value, max });
                    }
                }
            }
        }
        None => {
            for (entity, ..) in placed {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Says `text` where the screen shows a refusal: the editor's status, the launcher's problem line.
fn tell(text: &str, editor: Option<&mut crate::editor::EditorState>, launcher: Option<&mut crate::launcher::Launcher>) {
    if let Some(editor) = editor {
        editor.status.problem(text);
    }
    if let Some(launcher) = launcher {
        launcher.problem = Some(text.to_owned());
    }
}

fn dirty(open_map: Option<&crate::project::OpenMap>) -> bool {
    open_map.is_some_and(crate::project::OpenMap::dirty)
}

fn on_activate(
    _: On<bevy_gooey::Activate>,
    updater: Option<ResMut<Updater>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    mut editor: Option<ResMut<crate::editor::EditorState>>,
    mut launcher: Option<ResMut<crate::launcher::Launcher>>,
) {
    let Some(mut updater) = updater else { return };
    if let Some(reason) = updater.blocked.clone() {
        tell(&reason, editor.as_deref_mut(), launcher.as_deref_mut());
        return;
    }
    match updater.state.clone() {
        UpdateState::Idle | UpdateState::UpToDate | UpdateState::Failed(_) => updater.begin_check(true),
        UpdateState::Available(offer) => {
            if dirty(open_map.as_deref()) {
                tell(SAVE_FIRST, editor.as_deref_mut(), launcher.as_deref_mut());
            } else {
                updater.begin_download(offer);
            }
        }
        UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Installing(_) => {}
    }
}

/// What a translocated app says instead of updating.
pub const MOVE_IT_FIRST: &str =
    "Cauldron is running from the disk image or a quarantined folder: drag it into Applications, open it from there, then update";

/// **Inserts the [`Updater`] when this process is an installed macOS app.** Called by the binary only: the
/// feed is [`FEED_ENV`]'s value or [`FEED`]; a [`Bundle::Translocated`] app gets one that says
/// [`MOVE_IT_FIRST`]; a bare binary gets none.
pub fn insert_for_the_running_app(app: &mut App) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let feed = std::env::var(FEED_ENV).unwrap_or_else(|_| FEED.to_owned());
    match bundle_of(&exe) {
        Bundle::Installed(bundle) => {
            app.insert_resource(Updater::new(feed, bundle));
        }
        Bundle::Translocated(bundle) => {
            let mut updater = Updater::new(feed, bundle);
            updater.blocked = Some(MOVE_IT_FIRST.to_owned());
            app.insert_resource(updater);
        }
        Bundle::NotABundle => {}
    }
}

pub struct UpdatePlugin;

impl Plugin for UpdatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                start_the_first_check,
                // **Progress first, so the bar and the label read this frame's bytes** rather than the
                // last frame's.
                progress,
                poll,
                place_in_the_chrome_bar.run_if(resource_exists::<crate::chrome::Frame>),
                relabel,
                show_the_bar,
                size_the_bar,
            )
                .chain(),
        );
    }
}

/// The check every launch makes, once, unless the app cannot update where it is.
fn start_the_first_check(updater: Option<ResMut<Updater>>, mut done: Local<bool>) {
    let Some(mut updater) = updater else { return };
    if *done {
        return;
    }
    *done = true;
    if updater.state == UpdateState::Idle && updater.blocked.is_none() {
        // Nobody pressed for this one, so "nothing newer" is not news. See [`Updater::manual`].
        updater.begin_check(false);
    }
}

/// **Writes a download's progress into [`Updater`] every frame**, so a bar can follow it.
///
/// A no-op unless a download is in flight: the channel is made in `begin_download` and dropped when
/// the job answers, and taking from an empty one costs a lock.
fn progress(updater: Option<ResMut<Updater>>) {
    let Some(mut updater) = updater else { return };
    updater.take_progress();
}

/// Gives a newly spawned bar its size, then drops the mark: the node is `meter`'s own, so nothing else
/// may own it.
fn size_the_bar(mut commands: Commands, mut fresh: Query<(Entity, &mut Node), With<UpdateBarSize>>) {
    for (entity, mut node) in &mut fresh {
        node.width = Val::Px(UPDATE_BAR_W);
        node.height = Val::Px(UPDATE_BAR_H);
        commands.entity(entity).remove::<UpdateBarSize>();
    }
}

/// Takes a job's answer when it has one.
fn poll(
    updater: Option<ResMut<Updater>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    mut editor: Option<ResMut<crate::editor::EditorState>>,
    mut launcher: Option<ResMut<crate::launcher::Launcher>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut updater) = updater else { return };
    let answer = match updater.rx.as_ref() {
        None => return,
        Some(rx) => match rx.lock() {
            Ok(rx) => rx.try_recv(),
            Err(_) => Err(TryRecvError::Disconnected),
        },
    };
    let msg = match answer {
        Ok(msg) => msg,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Msg::Staged(Err("the update stopped without an answer".to_owned())),
    };
    updater.rx = None;
    // The download is over, so its progress channel is too: a later check must not read yesterday's
    // bytes.
    updater.progress_rx = None;
    // **The flag belongs to the job that just answered**, and the take below reads it once.
    let asked = updater.manual;
    updater.manual = false;
    let failed = match msg {
        Msg::Checked(Ok(Some(offer))) => {
            updater.state = UpdateState::Available(offer);
            None
        }
        Msg::Checked(Ok(None)) => {
            // **An answer with no words reads as a press that did nothing** — so a check a person
            // pressed for says which version is newest. **A check nobody pressed for says nothing**:
            // it is the label's job (`Up to date — v…`), and a sticky toast on every launch is a
            // receipt in the danger skin sitting where a real refusal would appear.
            updater.state = UpdateState::UpToDate;
            asked.then(|| {
                format!(
                    "Cauldron v{} is the newest release.",
                    env!("CARGO_PKG_VERSION")
                )
            })
        }
        Msg::Checked(Err(e)) => {
            // **A failure speaks whoever asked for it**: a launch that cannot reach the feed is a
            // fact the person needs, and it names itself (`Update failed — try again`).
            warn!("update check failed: {e}");
            updater.state = UpdateState::Failed(e);
            None
        }
        Msg::Staged(Ok(new)) => {
            let offer = updater.staging.take();
            match offer {
                // Edited while it downloaded: the swap would quit over unsaved work, so it is offered again.
                Some(offer) if dirty(open_map.as_deref()) => {
                    updater.state = UpdateState::Available(offer);
                    Some(SAVE_FIRST.to_owned())
                }
                Some(offer) => {
                    updater.state = UpdateState::Installing(offer.version);
                    match swap(&new, &updater.bundle) {
                        Ok(()) => {
                            exit.write(AppExit::Success);
                            None
                        }
                        Err(e) => {
                            updater.state = UpdateState::Failed(e.clone());
                            Some(e)
                        }
                    }
                }
                None => {
                    let e = "an update arrived that nobody asked for".to_owned();
                    updater.state = UpdateState::Failed(e.clone());
                    Some(e)
                }
            }
        }
        // **Progress on this channel cannot happen**, because it is a different one — but a match
        // that cannot happen still has to be written, and treating it as an answer would end the job.
        Msg::Progress { .. } => return,
        Msg::Staged(Err(e)) => {
            updater.staging = None;
            updater.state = UpdateState::Failed(e.clone());
            Some(e)
        }
    };
    if let Some(said) = failed {
        // **Said is not the same as wrong.** An up-to-date answer and a failure both go to the
        // person's own line, and only a failure is a warning in the log.
        if matches!(updater.state, UpdateState::Failed(_)) {
            warn!("update: {said}");
        }
        tell(&said, editor.as_deref_mut(), launcher.as_deref_mut());
    }
}

/// Keeps one [`UpdateButton`] at the end of the chrome bar while [`Updater::in_chrome`], and none otherwise.
fn place_in_the_chrome_bar(
    mut commands: Commands,
    frame: Res<crate::chrome::Frame>,
    updater: Option<Res<Updater>>,
    nodes: Query<(), With<Node>>,
    buttons: Query<(Entity, &ChildOf), With<UpdateButton>>,
) {
    // The frame outlives its entities across a screen change; a bar that is gone has nothing to hold.
    if nodes.get(frame.chrome_bar).is_err() {
        return;
    }
    let placed: Vec<Entity> = buttons.iter().filter(|(_, p)| p.parent() == frame.chrome_bar).map(|(e, _)| e).collect();
    match updater.as_deref().filter(|u| u.in_chrome()) {
        Some(updater) if placed.is_empty() => {
            commands.entity(frame.chrome_bar).with_child(update_button(updater.label()));
        }
        Some(_) => {}
        None => {
            for button in placed {
                commands.entity(button).despawn();
            }
        }
    }
}

/// Every update button reads [`Updater::label`].
fn relabel(updater: Option<Res<Updater>>, buttons: Query<&Children, With<UpdateButton>>, mut texts: Query<&mut Text>) {
    let Some(updater) = updater else { return };
    let want = updater.label();
    for child in buttons.iter().flat_map(|children| children.iter()) {
        if let Ok(mut text) = texts.get_mut(child)
            && text.0 != want
        {
            text.0.clone_from(&want);
        }
    }
}
