//! **Palette previews** — one baked render of every descriptor in the library.
//!
//! A palette of ids tells you what a piece is *called*, which is not the question an author is
//! asking. `crt_a` and `crt_b` differ in a way no name conveys, and a library of hundreds is
//! unusable without pictures.
//!
//! # The photo booth
//!
//! Staging happens at [`BOOTH`], four kilometres from the origin, so the booth camera sees the staged
//! piece and nothing else. No `RenderLayers`, which is where layer-based masking usually goes wrong
//! on GLB scene children. One camera walks the library: stage a piece, wait for it to be genuinely
//! ready, aim, render into that piece's image, move on — then the camera **despawns**, because a live
//! render target costs a full pass every frame forever.
//!
//! # Two traps this was built already knowing about
//!
//! Both were paid for in the game's editor and are the reason this is not a naive port.
//!
//! **A `Mesh3d` component is not a mesh that can be drawn.** The component appears as soon as the
//! scene instantiates; the mesh and material *assets* may still be uploading. So the gate is both
//! signals — the component exists AND `is_loaded_with_dependencies`.
//!
//! **Never leave the camera aimed at a finished thumbnail with nothing staged.** This system runs in
//! `Update`, so a despawn lands before the renderer extracts and the next frame draws an empty booth
//! through whatever image the camera still points at. In the game that silently cleared three
//! specific pieces — whichever ones the following GLB was too slow to repaint — and the log
//! cheerfully reported all 45 baked. Here the camera parks on [`Thumbnails::scratch`] whenever no
//! subject is standing there, so a real image is its target only while its piece is present.
//!
//! # The disk cache
//!
//! Baking 120 portraits takes minutes of staging, and the editor used to do it on every launch. A
//! finished portrait is written to `<project root>/target/grimoire_thumbs/<key>.png` (the
//! `target/vlm_suggestions.ron` precedent: derived, regenerable, gitignored) and read back on the
//! next launch instead of being baked. The key is a hash of everything the picture depends on: the
//! descriptor id, mesh path, file size and mtime, its `align`, the portrait size and
//! [`CACHE_VERSION`], which is bumped when the booth's framing or lights change.
//!
//! # Order
//!
//! The booth takes the pieces the palette shows first ([`ThumbOrder`], written by the palette
//! when it rebuilds), then the rest in library order, so the rows an author is looking at fill in
//! before the folded ones.

use bevy::camera::{ClearColorConfig, ImageRenderTarget, RenderTarget, ScalingMode};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use std::path::{Path, PathBuf};

use crate::project::Project;

/// Where pieces are staged for their portrait — far enough that nothing else can wander into frame.
const BOOTH: Vec3 = crate::stages::THUMB_BOOTH;

/// Thumbnail edge in pixels. Small enough that a few hundred are trivial memory, large enough to
/// tell a stool from a chair.
const THUMB_PX: u32 = 128;

/// Frames to hold a staged piece after it is ready. Wide enough that a render-target change landing a
/// frame late still leaves good frames inside the window.
const SETTLE_FRAMES: u32 = 4;

/// Camera distance as a multiple of the subject's largest dimension.
const FRAMING: f32 = 1.7;

/// How long to wait for a staged GLB before giving up on it. Generous but bounded: one unloadable
/// asset must not leave every later row blank.
const MESH_WAIT_FRAMES: u32 = 600;

/// An opaque tile rather than a transparent cut-out, so a row reads the same at rest, hovered and
/// armed — and so thin or glazed geometry is not composited away to nothing. The palette's empty
/// slot is the same ground under a different role, so the value is stated once, in chrome.
const BACKDROP: Color = crate::chrome::SLOT_BG;

/// The booth camera. Every other camera query filters positively on `view::MainCamera`, which is what
/// stops this one breaking them; this marker is just how the baker finds its own camera back.
#[derive(Component)]
pub struct ThumbnailCamera;

/// The staged subject, carrying its scene handle so [`bake`] can ask whether it is ready to draw.
/// `pub(crate)`: the label booth stages subjects through the same machinery.
#[derive(Component)]
pub(crate) struct Subject(pub(crate) Handle<WorldAsset>);

#[derive(Resource)]
pub struct Thumbnails {
    /// **Keyed by descriptor id, never by index.**
    ///
    /// This was a `Vec` parallel to the library, sized once at startup, and both ways the library can
    /// change broke it. Accepting a piece grew the library past the end of the vector, so the new row
    /// looked up nothing and showed a blank tile — and the booth had already torn itself down, so it
    /// could never be filled. Removing a piece was worse: every index after it shifted, and the
    /// palette silently showed each remaining row its *neighbour's* portrait.
    ///
    /// An id is what a row actually is. Handles are created for every descriptor as soon as it exists,
    /// so the palette has something to bind before anything is rendered — a row shows an empty tile
    /// until its turn comes, which is the same behaviour as before for the startup case.
    images: std::collections::HashMap<String, Handle<Image>>,
    /// Which ids have actually been rendered. Separate from [`Self::images`], which only says a
    /// handle exists: the difference is exactly "what is left to do", and it is what lets the booth
    /// come back when a piece is added rather than being finished forever.
    baked: std::collections::HashSet<String>,
    /// A throwaway target that absorbs every frame in which no subject is staged. See the module note.
    scratch: Handle<Image>,
    /// The id being staged right now, so a completed bake marks the piece it was actually for.
    staging: Option<String>,
    model: Option<Entity>,
    settled: u32,
    waited: u32,
    camera: Option<Entity>,
    booth: Option<Entity>,
    /// The cache readback of the portrait being taken, while there is one.
    capture: Option<Capture>,
}

impl Thumbnails {
    pub fn image(&self, id: &str) -> Option<Handle<Image>> {
        self.images.get(id).cloned()
    }

    /// Whether this id has a portrait — rendered this session or read from the disk cache.
    pub fn is_baked(&self, id: &str) -> bool {
        self.baked.contains(id)
    }

    /// **The next descriptor with no portrait yet, if any** — the whole of "what is left to do".
    ///
    /// The first unbaked id in `order` that the library still holds (the palette's own order), then
    /// the first unbaked in library order. The fallback is what finishes the pieces `order` leaves
    /// out: folded categories, and every piece before the palette has ever been built.
    fn pending(&self, library: &bevy_grimoire_core::library::Library, order: &[String]) -> Option<usize> {
        for id in order {
            if self.baked.contains(id.as_str()) {
                continue;
            }
            if let Some(ix) = library.descriptors.iter().position(|d| d.id == *id) {
                return Some(ix);
            }
        }
        library
            .descriptors
            .iter()
            .position(|d| !self.baked.contains(&d.id))
    }
}

/// **Bumped only when a portrait handle is first created for an id.**
///
/// The palette binds handles when it is built, so it has to be rebuilt once after a new piece gets
/// one — but gating that on `resource_changed::<Thumbnails>` would rebuild all forty rows on every
/// frame of the bake, because `bake` takes it as `ResMut` and derefs it each run. This changes a few
/// times per session instead of a few hundred times per startup.
#[derive(Resource, Default)]
pub struct ThumbGeneration(pub u32);

/// **The order the palette shows its rows in**, as descriptor ids — what the booth bakes first.
///
/// Written by the palette's rebuild and only then, so a bake that is under way is not reshuffled by
/// anything but the author changing what the palette shows.
#[derive(Resource, Default)]
pub struct ThumbOrder(pub Vec<String>);

/// Bumped whenever the booth's framing, lighting or backdrop changes, so a cached portrait taken
/// under the old rules is not served as if it were taken under the new ones.
const CACHE_VERSION: u32 = 1;

/// **Where this descriptor's cached portrait lives**, or `None` when it cannot have one.
///
/// A descriptor with no mesh has no portrait to cache, and a mesh that cannot be statted is not one
/// to trust a cache for. `pub` so a test can write the file the next launch should find.
pub fn cache_path(root: &Path, d: &bevy_grimoire_core::descriptor::Descriptor) -> Option<PathBuf> {
    let mesh = d.mesh.as_deref()?;
    let meta = std::fs::metadata(root.join("assets").join(mesh)).ok()?;
    let mtime_ns = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let identity = format!(
        "{}|{mesh}|{}|{mtime_ns}|{:?}|{THUMB_PX}|{CACHE_VERSION}",
        d.id,
        meta.len(),
        d.align
    );
    let key = bevy_grimoire_core::glb::fnv1a(identity.as_bytes());
    Some(root.join("target").join("grimoire_thumbs").join(format!("{key:016x}.png")))
}

/// The cached portrait at `path` as an image, or `None` — silently when the file is not there, and
/// with a warning when it is there and cannot be used (the piece is then baked as if it had none).
fn load_cached(path: &Path) -> Option<Image> {
    let bytes = std::fs::read(path).ok()?;
    let decoded = match image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            warn!("{}: unreadable cached portrait, baking it again: {e}", path.display());
            return None;
        }
    };
    if decoded.dimensions() != (THUMB_PX, THUMB_PX) {
        warn!(
            "{}: cached portrait is {}x{}, not {THUMB_PX}x{THUMB_PX}; baking it again",
            path.display(),
            decoded.width(),
            decoded.height()
        );
        return None;
    }
    Some(Image::new(
        Extent3d {
            width: THUMB_PX,
            height: THUMB_PX,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        decoded.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

/// A readback of the portrait being taken, in flight.
///
/// **A screenshot of an image target is a re-render, not a copy.** Bevy redirects whatever camera
/// draws into that image this frame into the capture's own texture and reads that back (see
/// `prepare_screenshots`' `view_target_attachments`). So the request has to be made while the booth
/// camera is still aimed at the portrait: asked for after parking it, the capture is of a frame nothing
/// drew into (all zeros), which is what the first version wrote to the cache. The portrait itself
/// keeps what the earlier settle frames drew; only the captured frame goes to the capture texture.
struct Capture {
    screenshot: Entity,
    /// Set by the observer once the PNG is written (or has failed, which is a missing optimisation
    /// and no reason to hold the booth).
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    waited: u32,
}

/// Frames to wait for a capture before giving the portrait up for the cache. The copy lands a frame
/// or two after the request; this is for a machine with no GPU path to read back from.
const CAPTURE_WAIT_FRAMES: u32 = 120;

/// Ask for the readback of `handle`, which the booth camera is drawing into right now.
fn request_capture(commands: &mut Commands, handle: Handle<Image>, path: PathBuf) -> Capture {
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = done.clone();
    let screenshot = commands
        .spawn(Screenshot::image(handle))
        .observe(move |shot: On<ScreenshotCaptured>| {
            write_cached(&shot.image, &path);
            flag.store(true, std::sync::atomic::Ordering::Release);
        })
        .id();
    Capture { screenshot, done, waited: 0 }
}

/// PNG-encode `image` and write it through [`bevy_grimoire_core::atomic_file::write_cache`], so a
/// crash mid-write leaves no half a file for the next launch to decode and the cache costs no
/// flush per entry. A failure is a missing optimisation, never an error: it is said once and the
/// editor carries on baking.
fn write_cached(image: &Image, path: &Path) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let written = (|| -> Result<(), String> {
        let png = crate::labels::encode_png(image)?;
        bevy_grimoire_core::atomic_file::write_cache(path, png).map_err(|e| format!("{}: {e}", path.display()))
    })();
    if let Err(e) = written
        && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        warn!("palette thumbnails will not be cached: {e}");
    }
}

pub struct ThumbsPlugin;

impl Plugin for ThumbsPlugin {
    fn build(&self, app: &mut App) {
        // **On the Maps door `setup` is registered by the editor's own chain, not here**: the
        // palette binds these handles, so it must run after them, and one chain is easier to be sure
        // of than two plugins agreeing about order. Every other door has no palette and no chain, so
        // it registers `setup` itself — `prune` below takes a bare `ResMut<Thumbnails>`, which
        // panics its system in Bevy 0.19 when nobody built it.
        //
        // A run condition, not a build-time read: the door is chosen at runtime now.
        app.add_systems(
            OnEnter(crate::screen::Screen::Editor),
            setup.run_if(|door: Option<Res<crate::tiles::Door>>| {
                door.is_some_and(|d| *d != crate::tiles::Door::Map)
            }),
        );
        app.init_resource::<ThumbGeneration>()
            .init_resource::<ThumbOrder>()
            // Chained so an eviction is visible to the same frame's bake — a pruned id must not be
            // staged one more time off a stale `pending` answer.
            .add_systems(Update,
                ((prune, bake.run_if(unfinished)).chain())
                    .run_if(in_state(crate::screen::Screen::Editor)),
            );
    }
}

/// Build the handles and stand the booth up. First link of the editor's Startup chain.
pub fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(Thumbnails {
        images: std::collections::HashMap::new(),
        baked: std::collections::HashSet::new(),
        scratch: blank_target(&mut images),
        staging: None,
        model: None,
        settled: 0,
        waited: 0,
        camera: None,
        booth: None,
        capture: None,
    });
}

/// A 128 px render target, zeroed.
///
/// `new_target_texture` sets the RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_DST usage a camera target
/// needs; hand-building the descriptor is the usual way to a silently black thumbnail. **COPY_SRC is
/// added**, because the disk cache reads a finished portrait back with a screenshot, which is a
/// texture-to-buffer copy with this image as the source — without it the copy is a wgpu validation
/// error that takes the frame's render with it, and every palette portrait came out blank (the label
/// booth's `shot_target` is the precedent).
fn blank_target(images: &mut Assets<Image>) -> Handle<Image> {
    let mut img = Image::new_target_texture(THUMB_PX, THUMB_PX, TextureFormat::Rgba8UnormSrgb, None);
    img.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    img.data = Some(vec![0; (THUMB_PX * THUMB_PX * 4) as usize]);
    images.add(img)
}

/// **Forget the portraits of pieces that no longer exist.**
///
/// The caches (`images`, `baked`) were insert-only, and that resurrected the wrong-picture bug the id
/// keying was built to end, one step removed: remove a tile and re-import a different mesh under the
/// same id — or redo an undone accept — and `baked` still holds the id, so [`Thumbnails::pending`]
/// skips it and the palette shows the OLD mesh's portrait for the new piece, permanently.
///
/// Change-gated inside the body rather than by a `resource_changed` run condition, because `Project`
/// is not this plugin's to assume: a run condition on a missing resource panics in 0.19, and taking
/// `Option<Res>` keeps the standalone plugin bootable.
fn prune(mut thumbs: ResMut<Thumbnails>, project: Option<Res<Project>>) {
    let Some(project) = project else { return };
    if !project.is_changed() {
        return;
    }
    let live = |id: &str| project.library.descriptors.iter().any(|d| d.id == id);
    // Checked before any mutable touch, so the common frame does not mark the resource changed.
    let anything_dead = thumbs.images.keys().any(|id| !live(id))
        || thumbs.baked.iter().any(|id| !live(id));
    if !anything_dead {
        return;
    }
    thumbs.images.retain(|id, _| live(id));
    thumbs.baked.retain(|id| live(id));
    // The staged model may belong to a pruned id; `bake`'s staging guard despawns it the moment the
    // pending piece disagrees, so nothing more is needed here.
}

/// `Option<Res<_>>`, never a bare `Res<_>`: Bevy 0.19 evaluates **every** run condition — there is no
/// short-circuit — and a missing resource in one panics the system at param validation. That shipped
/// to the game's `main` once and crashed every launch.
fn unfinished(
    thumbs: Option<Res<Thumbnails>>,
    project: Option<Res<Project>>,
    order: Option<Res<ThumbOrder>>,
) -> bool {
    // **Derived, not sticky.** This read a `finished` flag, so the booth was dismantled once and the
    // system never ran again — which is why a piece accepted after startup could never get a portrait.
    // Asking the library every frame costs a `position` over ~40 short strings and lets the booth return.
    match (thumbs, project) {
        (Some(t), Some(p)) => {
            let order = order.as_deref().map_or(&[][..], |o| o.0.as_slice());
            t.pending(&p.library, order).is_some() || t.camera.is_some()
        }
        _ => false,
    }
}

/// **Stand a booth up at `at`.** Called whenever there is a portrait to take and no booth standing
/// — at startup, and again after a piece is imported, which is the case the old one-shot `ensure`
/// could not serve. `pub(crate)` + positioned: the label booth raises the same three lights at its
/// own corner, so there is exactly one lighting recipe to keep true.
pub(crate) fn erect_booth(commands: &mut Commands, at: Vec3) -> Entity {
    // Point lights, so nothing outside their range is touched, and no shadows — a 128 px portrait
    // cannot show one and every shadow map costs a pass.
    
    commands
        .spawn((
            Name::new("photo booth"),
            Transform::from_translation(at),
            Visibility::Inherited,
        ))
        .with_children(|b| {
            for (offset, intensity) in [
                (Vec3::new(2.0, 3.0, 2.0), 400_000.0),
                (Vec3::new(-3.0, 2.0, 1.0), 150_000.0),
                (Vec3::new(0.0, 2.0, -3.0), 120_000.0),
            ] {
                b.spawn((
                    PointLight {
                        intensity,
                        range: 20.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(offset),
                ));
            }
        })
        .id()
}

#[allow(clippy::too_many_arguments)]
fn bake(
    mut commands: Commands,
    mut thumbs: ResMut<Thumbnails>,
    mut images: ResMut<Assets<Image>>,
    mut generation: ResMut<ThumbGeneration>,
    order: Res<ThumbOrder>,
    project: Res<Project>,
    assets: Res<AssetServer>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh3d>>,
    subjects: Query<&Subject>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    mut cams: Query<(&mut Transform, &mut Projection, &mut RenderTarget), With<ThumbnailCamera>>,
) {
    // Every descriptor gets a handle as soon as it exists, so the palette has something to bind
    // before this one's turn comes round. A piece with a cached portrait gets its picture here and
    // is done — before `pending` is asked, so it is never staged.
    for other in &project.library.descriptors {
        if !thumbs.images.contains_key(&other.id) {
            let cached = cache_path(&project.root, other)
                .as_deref()
                .and_then(load_cached);
            let handle = match cached {
                Some(image) => {
                    thumbs.baked.insert(other.id.clone());
                    images.add(image)
                }
                None => blank_target(&mut images),
            };
            thumbs.images.insert(other.id.clone(), handle);
            generation.0 = generation.0.wrapping_add(1);
        }
    }
    let Some(ix) = thumbs.pending(&project.library, &order.0) else {
        // Nothing left to take. The booth comes down, but the resource keeps every portrait it has —
        // and stands back up the moment a piece is imported.
        teardown(&mut commands, &mut thumbs);
        return;
    };
    let Some(d) = project.library.descriptors.get(ix).cloned() else {
        return;
    };
    if thumbs.booth.is_none() {
        thumbs.booth = Some(erect_booth(&mut commands, BOOTH));
    }

    // **The staged model must be the pending piece's, or it goes.** `pending` can switch subjects
    // mid-bake — the staged tile is removed on the Tiles tab, or an undo swaps the library — and the
    // loop below has no other way to know: it would frame whatever mesh is standing there and write
    // piece A's render into piece B's portrait, which is the wrong-picture bug the id keying exists
    // to end. `staging` names whose model `thumbs.model` is; it is written only here and at the
    // completed-portrait reset below.
    if thumbs.model.is_some() && thumbs.staging.as_deref() != Some(d.id.as_str()) {
        if let Some(model) = thumbs.model.take() {
            commands.entity(model).despawn();
        }
        thumbs.settled = 0;
        thumbs.waited = 0;
        if let Some(capture) = thumbs.capture.take() {
            commands.entity(capture.screenshot).try_despawn();
        }
        // Restaged for the right piece next frame.
        return;
    }
    thumbs.staging = Some(d.id.clone());

    let Some(model) = thumbs.model else {
        match stage(&mut commands, &assets, &d) {
            Some(model) => {
                thumbs.model = Some(model);
                thumbs.settled = 0;
                thumbs.waited = 0;
                if thumbs.camera.is_none() {
                    let scratch = thumbs.scratch.clone();
                    thumbs.camera = Some(spawn_camera(&mut commands, &scratch));
                }
            }
            // A descriptor with no mesh has no portrait, and that is not an error — it is a
            // descriptor that has not been given a mesh yet. Mark it done rather than stalling the
            // whole library behind it.
            None => {
                thumbs.baked.insert(d.id.clone());
            }
        }
        return;
    };

    let ready = subjects
        .get(model)
        .is_ok_and(|s| assets.is_loaded_with_dependencies(&s.0))
        && has_mesh(model, &children, &meshes);
    if !ready {
        thumbs.waited += 1;
        if thumbs.waited > MESH_WAIT_FRAMES {
            // Loud, and then keep walking: a piece whose GLB never instantiates would otherwise stall
            // the library behind it, leaving every later row blank with no clue why.
            warn!(
                "`{}` produced no drawable mesh in {MESH_WAIT_FRAMES} frames; its palette row will \
                 have no preview",
                d.id
            );
            commands.entity(model).despawn();
            thumbs.model = None;
            thumbs.baked.insert(d.id.clone());
        }
        return;
    }

    if let Some(cam_entity) = thumbs.camera
        && let Ok((mut tf, mut proj, mut target)) = cams.get_mut(cam_entity)
    {
        // **Frame what is actually there, not what the descriptor claims.**
        //
        // Aiming from `extent.footprint`/`height` assumes the geometry sits at the origin with
        // its base at zero, and plenty of meshes do not: `lamp_tall` is a ceiling fitting and
        // `wall_light` is a wall plate, both authored where they hang. Framing them from the
        // origin pointed the camera at empty air and baked two blank tiles — the same *symptom*
        // as the render-target race, from a completely different cause, which is the argument for
        // measuring rather than assuming twice over.
        let Some((lo, hi)) = subject_bounds(model, &children, &bounds) else {
            // Unreachable behind `has_mesh` above: a drawable mesh has an `Aabb`. Loud rather
            // than guessed, because the alternative is a camera aimed at a number the descriptor
            // claims and a baked picture of nothing — the exact failure the paragraph above says
            // measuring exists to prevent, reintroduced on the branch that gives up measuring.
            error!(
                "`{}` passed the readiness gate with nothing measurable under it; no preview",
                d.id
            );
            return;
        };
        let (centre, extent) = aim_and_span(lo, hi);
        *tf = Transform::from_translation(centre + Vec3::new(1.0, 0.85, 1.0) * extent * FRAMING)
            .looking_at(centre, Vec3::Y);
        *proj = Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: extent * 1.35,
            },
            ..OrthographicProjection::default_3d()
        });
        *target = RenderTarget::Image(ImageRenderTarget {
            handle: match thumbs.images.get(&d.id) {
                Some(h) => h.clone(),
                None => return,
            },
            scale_factor: 1.0,
        });
    }

    thumbs.settled += 1;
    if thumbs.settled <= SETTLE_FRAMES {
        return;
    }

    // **Take the cache readback before letting go**, with the camera still drawing into the portrait
    // (see [`Capture`]), and hold the booth on this piece until it lands or gives up.
    if let Some(capture) = thumbs.capture.as_mut() {
        capture.waited += 1;
        let landed = capture.done.load(std::sync::atomic::Ordering::Acquire);
        if !landed && capture.waited <= CAPTURE_WAIT_FRAMES {
            return;
        }
        if !landed {
            warn!("`{}`: the portrait readback never landed; it is not cached", d.id);
        }
        let screenshot = capture.screenshot;
        commands.entity(screenshot).try_despawn();
        thumbs.capture = None;
    } else if let (Some(path), Some(handle)) = (cache_path(&project.root, &d), thumbs.images.get(&d.id)) {
        let capture = request_capture(&mut commands, handle.clone(), path);
        thumbs.capture = Some(capture);
        return;
    }

    // Park on the scratch target BEFORE letting go of the subject — see the module note.
    if let Some(cam_entity) = thumbs.camera
        && let Ok((_, _, mut target)) = cams.get_mut(cam_entity)
    {
        *target = RenderTarget::Image(ImageRenderTarget {
            handle: thumbs.scratch.clone(),
            scale_factor: 1.0,
        });
    }
    commands.entity(model).despawn();
    thumbs.model = None;
    thumbs.staging = None;
    thumbs.baked.insert(d.id.clone());
}

/// **The world-space box of everything actually drawn under `root`** — `(low corner, high corner)`.
///
/// `None` when nothing under it has an `Aabb` yet. **Both booths refuse on it** — behind their
/// readiness gates a drawable mesh always has one, so a `None` there is a broken invariant and not a
/// cue to guess a size. The one legitimate reader of `None` is the mesh stage, which uses it as a
/// "nothing staged yet" sentinel and re-derives its own span. `pub(crate)`: the label booth frames
/// from the same measured bounds, so the two booths cannot drift apart on what "framed" means — and
/// since 2026-08-20 so does the mesh stage's camera.
///
/// **It returns the box rather than a centre and a diameter**, and that widening is the whole reason
/// the mesh stage could use it. A thumbnail booth points a camera at a subject from a fixed distance,
/// so one number is enough; an isometric stage has to know how far the subject spreads *sideways*
/// versus how tall it *stands*, because those two land on different axes of the viewport. Collapsing
/// to `max(x, y, z)` here would have forced the third caller to measure the world a second time.
pub(crate) fn subject_bounds(
    root: Entity,
    children: &Query<&Children>,
    bounds: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    let mut any = false;
    let mut queue = vec![root];
    while let Some(e) = queue.pop() {
        if let Ok((aabb, tf)) = bounds.get(e) {
            // The AABB is in the mesh's own space; the corners have to go through the world transform
            // before they mean anything, because the scene root may scale and rotate them.
            let c = Vec3::from(aabb.center);
            let h = Vec3::from(aabb.half_extents);
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        let corner = tf.transform_point(c + h * Vec3::new(sx, sy, sz));
                        lo = lo.min(corner);
                        hi = hi.max(corner);
                        any = true;
                    }
                }
            }
        }
        if let Ok(kids) = children.get(e) {
            queue.extend(kids.iter());
        }
    }
    any.then_some((lo, hi))
}

/// A measured box as the booths want it: where to aim, and how big across at its widest.
///
/// The floor keeps a flat decal from putting the camera inside itself.
pub(crate) fn aim_and_span(lo: Vec3, hi: Vec3) -> (Vec3, f32) {
    let size = hi - lo;
    ((lo + hi) * 0.5, size.x.max(size.y).max(size.z).max(0.05))
}

fn stage(
    commands: &mut Commands,
    assets: &AssetServer,
    d: &bevy_grimoire_core::descriptor::Descriptor,
) -> Option<Entity> {
    let mesh = d.mesh.as_ref()?;
    Some(stage_mesh(
        commands,
        assets,
        mesh,
        d.align.scale.unwrap_or(1.0),
        BOOTH,
    ))
}

/// Stage one mesh at `at`, at the scale a real placement applies — so any booth photographs the
/// piece at the size it will actually be rather than at whatever the artist exported.
/// `pub(crate)`: the label booth stages through this.
pub(crate) fn stage_mesh(
    commands: &mut Commands,
    assets: &AssetServer,
    mesh: &str,
    scale: f32,
    at: Vec3,
) -> Entity {
    let scene: Handle<WorldAsset> =
        assets.load(GltfAssetLabel::Scene(0).from_asset(mesh.to_owned()));
    commands
        .spawn((
            Name::new("booth subject"),
            Subject(scene.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(scale)),
            Visibility::Inherited,
        ))
        .with_child((WorldAssetRoot(scene), Transform::default()))
        .id()
}

fn spawn_camera(commands: &mut Commands, first_target: &Handle<Image>) -> Entity {
    commands
        .spawn((
            Name::new("thumbnail camera"),
            ThumbnailCamera,
            Camera3d::default(),
            Camera {
                // Before the main camera, so the palette shows this frame's bake rather than the last.
                order: -1,
                clear_color: ClearColorConfig::Custom(BACKDROP),
                ..default()
            },
            // `RenderTarget` IS its own component in 0.19 — one of `Camera`'s `#[require]`s rather
            // than a field on it.
            RenderTarget::Image(ImageRenderTarget {
                handle: first_target.clone(),
                scale_factor: 1.0,
            }),
            // Component form, so it lifts this camera's exposure only; the resource form is global.
            AmbientLight {
                brightness: 900.0,
                ..default()
            },
            Projection::from(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: 2.0,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_translation(BOOTH + Vec3::splat(3.0)).looking_at(BOOTH, Vec3::Y),
        ))
        .id()
}

/// `pub(crate)`: the label booth gates readiness on the same two signals (see the module note on
/// why `Mesh3d` alone is not a drawable mesh).
pub(crate) fn has_mesh(
    root: Entity,
    children: &Query<&Children>,
    meshes: &Query<(), With<Mesh3d>>,
) -> bool {
    let mut queue = vec![root];
    while let Some(e) = queue.pop() {
        if meshes.get(e).is_ok() {
            return true;
        }
        if let Ok(kids) = children.get(e) {
            queue.extend(kids.iter());
        }
    }
    false
}

fn teardown(commands: &mut Commands, thumbs: &mut Thumbnails) {
    if let Some(cam) = thumbs.camera.take() {
        commands.entity(cam).despawn();
        // Logged because the bake is otherwise invisible: if a piece never instantiates, this line is
        // what says how far it got.
        info!("baked {} palette thumbnail(s); booth torn down", thumbs.images.len());
    }
    if let Some(booth) = thumbs.booth.take() {
        commands.entity(booth).despawn();
    }
    if let Some(model) = thumbs.model.take() {
        commands.entity(model).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_grimoire_core::descriptor::Descriptor;
    use bevy_grimoire_core::library::Library;

    fn library(ids: &[&str]) -> Library {
        Library {
            descriptors: ids
                .iter()
                .map(|id| Descriptor {
                    id: (*id).to_owned(),
                    ..Descriptor::default()
                })
                .collect(),
            ..Library::default()
        }
    }

    fn thumbs(baked: &[&str]) -> Thumbnails {
        Thumbnails {
            images: Default::default(),
            baked: baked.iter().map(|s| (*s).to_owned()).collect(),
            scratch: Handle::default(),
            staging: None,
            model: None,
            settled: 0,
            waited: 0,
            camera: None,
            booth: None,
            capture: None,
        }
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// **The booth takes what the palette shows first**, then everything else in library order.
    #[test]
    fn the_palette_order_goes_first_and_the_library_finishes_the_rest() {
        let lib = library(&["a", "b", "c", "d"]);
        let t = thumbs(&["a"]);
        assert_eq!(t.pending(&lib, &ids(&["c", "b"])), Some(2), "the palette's first unbaked");
        assert_eq!(
            t.pending(&lib, &ids(&["a", "ghost", "d"])),
            Some(3),
            "baked and stale ids are skipped, and the rest of the order still counts"
        );
        assert_eq!(
            t.pending(&lib, &ids(&["a"])),
            Some(1),
            "an order with nothing left falls back to library order"
        );
        assert_eq!(thumbs(&["a", "b", "c", "d"]).pending(&lib, &ids(&["d"])), None);
    }
}
