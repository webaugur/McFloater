//! Bevy face for Floaty McFloater — Avaturn GLB + **embedded animation** + speech morphs.

mod face;
mod hud;
mod scene;

use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::texture::ImagePlugin;
use bevy::window::{PresentMode, Window, WindowPlugin, WindowResolution};
use crossbeam_channel::{Receiver, Sender};
use face::{
    animate_body, animate_face, assets_dir, drive_face_morphs, spawn_face, tag_avatar_bones,
    FaceAssetConfig, FaceParts,
};
use hud::{spawn_hud, update_hud};
use scene::setup_scene;
use tracing::info;

pub use face::SpeakPhase;
pub use mcfloater_core::RuntimeState;

pub const RENDER_TARGET_FPS: u32 = 60;

#[derive(Debug, Clone)]
pub enum FaceEvent {
    SetState(RuntimeState),
    SetCaption(String),
    /// `ok` = brain reachable. `ha_control` = real C&C entities exist (not just API up).
    BrainStatus {
        ok: bool,
        ha_control: bool,
        detail: String,
    },
    Mouth(f32),
    Quit,
}

#[derive(Debug, Clone)]
pub enum FaceRequest {
    Speak(String),
    Ask(String),
    /// Push-to-talk listen window → STT → chat → speak.
    Listen,
    Quit,
}

#[derive(Resource, Clone)]
pub struct FaceBridge {
    pub events_rx: Receiver<FaceEvent>,
    pub requests_tx: Sender<FaceRequest>,
}

#[derive(Resource, Debug, Clone)]
pub struct FaceStatus {
    pub state: RuntimeState,
    pub caption: String,
    pub brain_ok: bool,
    /// True only when HA has switch/light/scene entities (real C&C).
    pub ha_control: bool,
    pub brain_detail: String,
    pub mouth: f32,
    pub speak_phase: SpeakPhase,
}

impl Default for FaceStatus {
    fn default() -> Self {
        Self {
            state: RuntimeState::Idle,
            caption: "FLOATY McFLOATER — Space speak · A ask · L listen · Esc quit".into(),
            brain_ok: false,
            ha_control: false,
            brain_detail: "brain: not checked".into(),
            mouth: 0.0,
            speak_phase: SpeakPhase::Closed,
        }
    }
}

#[derive(Resource, Clone)]
pub struct FaceLines {
    pub demo: String,
    pub ask: String,
}

impl Default for FaceLines {
    fn default() -> Self {
        // Keep in sync with mcfloater_tts::DEMO_LINE / DEFAULT_ASK_LINE (face_host overrides).
        Self {
            demo: "Hello! I'm Floaty McFloater. Catch the wave — and welcome to the future!"
                .into(),
            // A = ask brain (this is the *question*, not the spoken line).
            ask: "Hello!".into(),
        }
    }
}

pub fn run_face(
    events_rx: Receiver<FaceEvent>,
    requests_tx: Sender<FaceRequest>,
    lines: FaceLines,
) {
    let assets = assets_dir();
    info!(assets = %assets.display(), "starting Bevy face (GLB animation + morphs)");

    // Sharp defaults: higher window + MSAA + linear sampler with anisotropy.
    // Note: Avaturn GLB maps themselves top out at 1024² — not downscaled by us.
    let mut default_sampler = ImageSamplerDescriptor::linear();
    default_sampler.anisotropy_clamp = 16;

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Floaty McFloater".into(),
                        // Was 960×720 — face was a small fraction of a soft window.
                        resolution: WindowResolution::new(1600.0, 1200.0),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: assets.to_string_lossy().into_owned(),
                    ..default()
                })
                .set(ImagePlugin {
                    default_sampler,
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.06)))
        .insert_resource(FaceBridge {
            events_rx,
            requests_tx,
        })
        .insert_resource(FaceStatus::default())
        .insert_resource(FaceParts::default())
        .insert_resource(FaceAssetConfig::default())
        .insert_resource(lines)
        .insert_resource(AmbientLight {
            color: Color::srgb(0.92, 0.90, 0.90),
            brightness: 55.0,
        })
        .add_systems(Startup, (setup_scene, spawn_face, spawn_hud))
        .add_systems(
            Update,
            (
                boost_loaded_texture_anisotropy,
                tag_avatar_bones,
                drain_events,
                keyboard_controls,
                animate_body,
                animate_face,
                update_hud,
                exit_on_quit_event,
            ),
        )
        // CRITICAL: glTF weight animation runs in PostUpdate (animate_targets → inherit_weights).
        // If we set morphs in Update, the clip zeroes them again. Drive speech AFTER that.
        .add_systems(
            PostUpdate,
            drive_face_morphs
                .after(bevy::animation::animate_targets)
                .after(bevy::render::mesh::inherit_weights),
        )
        .run();
}

/// Avaturn glTF samplers leave `anisotropy_clamp = 1`. Full-body framing minifies
/// face maps hard — 16× AF helps. wgpu requires **all** filter modes Linear when
/// AF > 1 (some glTF maps use Linear min/mag but Nearest mipmap → crash if we only
/// bump AF).
///
/// Scan with `iter()` first so we only `get_mut` images that still need a boost.
fn boost_loaded_texture_anisotropy(mut images: ResMut<Assets<Image>>) {
    const TARGET: u16 = 16;
    let needs_boost: Vec<_> = images
        .iter()
        .filter_map(|(id, image)| {
            if let ImageSampler::Descriptor(desc) = &image.sampler {
                // Mag must already be linear (don't AF nearest-pixel UI-style maps).
                if desc.anisotropy_clamp < TARGET
                    && matches!(desc.mag_filter, ImageFilterMode::Linear)
                {
                    return Some(id);
                }
            }
            None
        })
        .collect();

    for id in needs_boost {
        if let Some(image) = images.get_mut(id) {
            if let ImageSampler::Descriptor(desc) = &mut image.sampler {
                desc.min_filter = ImageFilterMode::Linear;
                desc.mag_filter = ImageFilterMode::Linear;
                desc.mipmap_filter = ImageFilterMode::Linear;
                desc.anisotropy_clamp = TARGET;
            }
        }
    }
}

fn drain_events(bridge: Res<FaceBridge>, mut status: ResMut<FaceStatus>) {
    while let Ok(ev) = bridge.events_rx.try_recv() {
        match ev {
            FaceEvent::SetState(state) => {
                status.state = state;
                if state != RuntimeState::Speaking {
                    status.mouth = 0.0;
                    status.speak_phase = SpeakPhase::Closed;
                }
            }
            FaceEvent::SetCaption(c) => status.caption = c,
            FaceEvent::BrainStatus {
                ok,
                ha_control,
                detail,
            } => {
                status.brain_ok = ok;
                status.ha_control = ha_control;
                status.brain_detail = detail;
            }
            FaceEvent::Mouth(m) => {
                status.mouth = m.clamp(0.0, 1.0);
                status.speak_phase = if status.mouth > 0.15 {
                    SpeakPhase::Open
                } else {
                    SpeakPhase::Closed
                };
            }
            FaceEvent::Quit => {
                status.caption = "QUIT".into();
            }
        }
    }
}

fn keyboard_controls(
    keys: Res<ButtonInput<KeyCode>>,
    bridge: Res<FaceBridge>,
    lines: Res<FaceLines>,
    status: Res<FaceStatus>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        let _ = bridge.requests_tx.send(FaceRequest::Quit);
    }
    if keys.just_pressed(KeyCode::Space) && status.state != RuntimeState::Speaking {
        let _ = bridge.requests_tx.send(FaceRequest::Speak(lines.demo.clone()));
    }
    if keys.just_pressed(KeyCode::KeyA) && status.state != RuntimeState::Speaking {
        let _ = bridge.requests_tx.send(FaceRequest::Ask(lines.ask.clone()));
    }
    if keys.just_pressed(KeyCode::KeyL)
        && status.state != RuntimeState::Speaking
        && status.state != RuntimeState::Listening
    {
        let _ = bridge.requests_tx.send(FaceRequest::Listen);
    }
}

fn exit_on_quit_event(status: Res<FaceStatus>, mut app_exit: EventWriter<AppExit>) {
    if status.caption == "QUIT" {
        app_exit.send(AppExit::Success);
    }
}
