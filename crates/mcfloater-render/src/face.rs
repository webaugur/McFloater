//! Avaturn T2 face — play **embedded GLB animation** for body; morphs for speech.
//!
//! Default asset: `face/T2-avatar-with-animation-bevy.glb`
//! (contains clip `avaturn_animation`: skeleton rotations + morph weight tracks).
//!
//! We do **not** invent arm Euler poses — that fought the rig. Body motion comes
//! from the clip Avaturn already baked in (see docs.avaturn.me Mixamo/export flow).

use crate::FaceStatus;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::render::mesh::morph::MeshMorphWeights;
use bevy::scene::SceneInstanceReady;
use mcfloater_core::RuntimeState;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Component)]
pub struct FaceRoot;

#[derive(Component)]
pub struct FaceHead;

#[derive(Component)]
pub struct FaceMouth;

#[derive(Component)]
pub struct ProceduralFace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeakPhase {
    #[default]
    Closed,
    Open,
}

#[derive(Resource, Default)]
pub struct FaceParts {
    pub mouth: Option<Entity>,
    pub head: Option<Entity>,
    pub has_morphs: bool,
}

/// Handle to the embedded body/face clip graph.
#[derive(Resource)]
pub struct FaceAnimation {
    pub graph: Handle<AnimationGraph>,
    pub node: AnimationNodeIndex,
    /// Asset path used (for logging).
    pub path: String,
}

#[derive(Resource, Clone, Debug)]
pub struct FaceAssetConfig {
    pub glb: String,
}

impl Default for FaceAssetConfig {
    fn default() -> Self {
        // Prefer T2 + embedded animation (user already supplied this).
        let glb = std::env::var("MCFLOATER_FACE_GLB").unwrap_or_else(|_| {
            "face/T2-avatar-with-animation-bevy.glb".into()
        });
        Self { glb }
    }
}

// Lips moderate; teeth/jaw open more so molars separate without stretching lips.
// Always write jaw/mouth every frame (including 0) so values don't stick after speech.
const MOUTH_GAIN: f32 = 0.12; // Head_Mesh lips — leave alone
const JAW_GAIN: f32 = 0.21; // Head_Mesh jaw skin
const TEETH_JAW_GAIN: f32 = 0.72; // Teeth_Mesh jawOpen — stronger unclench
const TEETH_MOUTH_GAIN: f32 = 0.40;
const BLINK_GAIN: f32 = 0.9;

pub fn assets_dir() -> PathBuf {
    if let Ok(p) = std::env::var("MCFLOATER_ASSETS") {
        return PathBuf::from(p);
    }
    let from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    if from_manifest.is_dir() {
        return from_manifest.canonicalize().unwrap_or(from_manifest);
    }
    let cwd = PathBuf::from("assets");
    if cwd.is_dir() {
        return cwd.canonicalize().unwrap_or(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for cand in [parent.join("assets"), parent.join("../assets")] {
                if cand.is_dir() {
                    return cand.canonicalize().unwrap_or(cand);
                }
            }
        }
    }
    PathBuf::from("assets")
}

fn glb_on_disk(rel: &str) -> Option<PathBuf> {
    let p = assets_dir().join(rel);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

fn pick_face_asset(cfg: &FaceAssetConfig) -> Option<String> {
    let candidates = [
        cfg.glb.as_str(),
        "face/T2-avatar-with-animation-bevy.glb",
        "face/T2-avatar-with-animation.glb",
        "face/T2-avatar-bevy.glb",
        "face/T2-avatar.glb",
        "face/avatar-with-animation.glb",
        "face/avatar.glb",
    ];
    for c in candidates {
        if glb_on_disk(c).is_some() {
            return Some(c.to_string());
        }
    }
    None
}

/// Startup: load scene + build AnimationGraph from GLB animation index 0.
pub fn spawn_face(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut parts: ResMut<FaceParts>,
    cfg: Res<FaceAssetConfig>,
) {
    let Some(rel) = pick_face_asset(&cfg) else {
        warn!(assets = %assets_dir().display(), "no face GLB — procedural stand-in");
        spawn_procedural_fallback(&mut commands, &mut meshes, &mut materials, &mut parts);
        return;
    };

    info!(asset = %rel, assets = %assets_dir().display(), "loading face GLB + animation clip 0");

    // Clip 0 = first (usually only) animation in the file, e.g. `avaturn_animation`
    let clip = asset_server.load(GltfAssetLabel::Animation(0).from_asset(rel.clone()));
    let (graph, node) = AnimationGraph::from_clip(clip);
    let graph_handle = graphs.add(graph);
    commands.insert_resource(FaceAnimation {
        graph: graph_handle,
        node,
        path: rel.clone(),
    });

    // Full-body framed for bust; animation drives pose (not our Euler hacks)
    commands
        .spawn((
            FaceRoot,
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(rel))),
            // Full-body: feet near y=0, camera pulled back in scene.rs
            Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(1.0)),
            Visibility::default(),
        ))
        .observe(on_scene_ready_play_animation);
}

/// When the glTF scene finishes spawning, AnimationPlayer exists — start the clip.
fn on_scene_ready_play_animation(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    anim: Option<Res<FaceAnimation>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some(anim) = anim else {
        return;
    };

    // Walk descendants for AnimationPlayer (Bevy adds it on the animated root)
    let mut stack = vec![trigger.entity()];
    let mut found = None;
    while let Some(e) = stack.pop() {
        if players.contains(e) {
            found = Some(e);
            break;
        }
        if let Ok(ch) = children.get(e) {
            stack.extend(ch.iter().copied());
        }
    }

    let Some(player_entity) = found else {
        warn!("SceneInstanceReady but no AnimationPlayer in hierarchy");
        return;
    };

    let Ok(mut player) = players.get_mut(player_entity) else {
        return;
    };

    let mut transitions = AnimationTransitions::new();
    transitions
        .play(&mut player, anim.node, Duration::ZERO)
        .repeat();

    commands
        .entity(player_entity)
        .insert(AnimationGraphHandle(anim.graph.clone()))
        .insert(transitions);

    info!(
        path = %anim.path,
        entity = ?player_entity,
        "playing embedded GLB animation (loop)"
    );
}

/// Note morph presence (speech still layers jaw/viseme weights on top of the clip).
pub fn tag_avatar_bones(
    mut parts: ResMut<FaceParts>,
    morphs: Query<(), Added<MeshMorphWeights>>,
    named: Query<(Entity, &Name), Added<Name>>,
    mut commands: Commands,
) {
    if !parts.has_morphs && morphs.iter().next().is_some() {
        parts.has_morphs = true;
        info!("morph targets available — speech will drive jawOpen/mouthOpen/visemes");
    }
    for (entity, name) in &named {
        if name.as_str() == "Head" {
            commands.entity(entity).insert(FaceHead);
            parts.head = Some(entity);
        }
    }
}

/// One syllable-ish step of fake lip-sync (until real phoneme timing exists).
struct TalkFrame {
    /// 0 = closed lips, 1 = wide open
    open: f32,
    jaw: f32,
    viseme: Option<&'static str>,
    funnel: f32,
    pucker: f32,
    lower: f32,
}

/// Procedural talk: open/close + viseme shapes — NOT a static hang.
fn talk_frame(t: f32) -> TalkFrame {
    // Syllable clock ~5–7 Hz with irregularity
    let syll = t * 6.2 + (t * 1.7).sin() * 0.35;
    let phase = syll.rem_euclid(1.0);

    // Duty cycle: closed ~35%, opening, open peak, closing
    let (open_env, closed) = if phase < 0.32 {
        (0.0, true) // lips together between syllables
    } else if phase < 0.45 {
        let u = (phase - 0.32) / 0.13;
        (u * u, false) // ease open
    } else if phase < 0.62 {
        let wobble = 0.85 + 0.15 * (t * 22.0).sin();
        (wobble, false) // hold open with micro jitter
    } else if phase < 0.82 {
        let u = 1.0 - (phase - 0.62) / 0.20;
        (u * u, false) // close
    } else {
        (0.0, true)
    };

    // Second oscillator: occasional bilabial “pop” closed
    let pop = ((t * 2.3).sin() * 0.5 + 0.5) > 0.88;
    let open_env = if pop { 0.0 } else { open_env };

    // Pick viseme by syllable index — closed shapes when mouth closed
    const OPEN_V: &[&str] = &[
        "viseme_aa",
        "viseme_E",
        "viseme_I",
        "viseme_O",
        "viseme_U",
        "viseme_aa",
        "viseme_E",
    ];
    const CLOSED_V: &[&str] = &["viseme_PP", "viseme_SS", "viseme_nn", "viseme_FF", "viseme_sil"];

    let idx = syll.floor() as usize;
    let viseme = if closed || open_env < 0.12 {
        Some(CLOSED_V[idx % CLOSED_V.len()])
    } else {
        Some(OPEN_V[idx % OPEN_V.len()])
    };

    // Shape-specific jaw/lip scaling
    let (jaw_s, funnel, pucker, lower) = match viseme {
        Some("viseme_aa") => (1.0, 0.0, 0.0, 0.35),
        Some("viseme_E") => (0.75, 0.0, 0.0, 0.25),
        Some("viseme_I") => (0.45, 0.0, 0.15, 0.15),
        Some("viseme_O") => (0.7, 0.55, 0.2, 0.2),
        Some("viseme_U") => (0.5, 0.35, 0.55, 0.1),
        Some("viseme_PP") | Some("viseme_sil") => (0.05, 0.0, 0.4, 0.0),
        Some("viseme_FF") | Some("viseme_SS") => (0.15, 0.0, 0.25, 0.05),
        Some("viseme_nn") => (0.2, 0.0, 0.1, 0.1),
        _ => (0.6, 0.0, 0.0, 0.2),
    };

    let open = open_env * if closed { 0.05 } else { 0.60 }; // peak ~2/3 of full open
    let jaw = open_env * jaw_s * 0.67;

    TalkFrame {
        open,
        jaw,
        viseme,
        funnel: funnel * open_env,
        pucker: pucker * open_env.max(if closed { 0.5 } else { 0.0 }),
        lower: lower * open_env,
    }
}

/// Speech morphs (after animation so talk wins over clip weight tracks).
pub fn drive_face_morphs(
    time: Res<Time>,
    status: Res<FaceStatus>,
    meshes: Res<Assets<Mesh>>,
    mut mesh_morphs: Query<(&mut MeshMorphWeights, &Mesh3d, Option<&Name>)>,
    mut parent_morphs: Query<(&mut MorphWeights, Option<&Name>)>,
) {
    let t = time.elapsed_secs();
    let speaking = status.state == RuntimeState::Speaking;

    // Do NOT floor with status.mouth (host used to send 0.8 constant → banana hang).
    // Build a real open/close + viseme sequence while Speaking.
    let frame = if speaking {
        talk_frame(t)
    } else {
        TalkFrame {
            open: 0.0,
            jaw: 0.0,
            viseme: Some("viseme_sil"),
            funnel: 0.0,
            pucker: 0.0,
            lower: 0.0,
        }
    };

    let lip = frame.open * MOUTH_GAIN;
    let jaw = frame.jaw * JAW_GAIN;
    let teeth_jaw = (frame.jaw * TEETH_JAW_GAIN).clamp(0.0, 1.0);
    let teeth_lip = (frame.open * TEETH_MOUTH_GAIN).clamp(0.0, 1.0);

    let phase = t % 4.0;
    let blink = if phase < 0.12 {
        let u = phase / 0.12;
        let s = if u < 0.5 { u * 2.0 } else { (1.0 - u) * 2.0 };
        s * BLINK_GAIN
    } else {
        0.0
    };

    let shape = TalkShape {
        open: lip,
        jaw,
        teeth_jaw,
        teeth_open: teeth_lip,
        viseme: frame.viseme,
        funnel: frame.funnel * MOUTH_GAIN,
        pucker: frame.pucker * MOUTH_GAIN,
        lower: frame.lower * JAW_GAIN,
        blink,
        speaking,
    };

    for (mut mw, mesh3d, name) in &mut mesh_morphs {
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };
        let kind = mesh_kind(name.map(|n| n.as_str()), mesh.morph_target_names());
        apply_weights(mw.weights_mut(), mesh.morph_target_names(), &shape, kind);
    }
    for (mut mw, name) in &mut parent_morphs {
        let names = mw
            .first_mesh()
            .and_then(|h| meshes.get(h))
            .and_then(|m| m.morph_target_names());
        let kind = mesh_kind(name.map(|n| n.as_str()), names);
        apply_weights(mw.weights_mut(), names, &shape, kind);
    }
}

struct TalkShape {
    open: f32,
    jaw: f32,
    teeth_jaw: f32,
    teeth_open: f32,
    viseme: Option<&'static str>,
    funnel: f32,
    pucker: f32,
    lower: f32,
    blink: f32,
    speaking: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MeshKind {
    Head,
    Teeth,
    Tongue,
    Other,
}

fn mesh_kind(name: Option<&str>, morph_names: Option<&[String]>) -> MeshKind {
    if let Some(n) = name {
        let l = n.to_ascii_lowercase();
        if l.contains("teeth") {
            return MeshKind::Teeth;
        }
        if l.contains("tongue") {
            return MeshKind::Tongue;
        }
        if l.contains("head") {
            return MeshKind::Head;
        }
    }
    // Infer from morph set: teeth-only has jawOpen + visemes, few ARKit brow shapes
    if let Some(names) = morph_names {
        let has_brow = names.iter().any(|n| n.starts_with("brow"));
        let has_jaw = names.iter().any(|n| n == "jawOpen");
        let has_tongue = names.iter().any(|n| n == "tongueOut");
        if has_jaw && has_tongue && !has_brow {
            return MeshKind::Tongue;
        }
        if has_jaw && !has_brow && names.len() <= 25 {
            return MeshKind::Teeth;
        }
        if has_brow {
            return MeshKind::Head;
        }
    }
    MeshKind::Other
}

fn apply_weights(
    weights: &mut [f32],
    names: Option<&[String]>,
    shape: &TalkShape,
    kind: MeshKind,
) {
    if weights.is_empty() {
        return;
    }

    let (open, jaw) = match kind {
        MeshKind::Teeth | MeshKind::Tongue => (shape.teeth_open, shape.teeth_jaw),
        MeshKind::Head | MeshKind::Other => (shape.open, shape.jaw),
    };

    let Some(names) = names else {
        weights[0] = open.clamp(0.0, 1.0);
        if weights.len() > 16 {
            weights[16] = jaw.clamp(0.0, 1.0);
        }
        return;
    };

    let set = |weights: &mut [f32], names: &[String], key: &str, v: f32| {
        if let Some(i) = names.iter().position(|n| n == key) {
            if i < weights.len() {
                weights[i] = v.clamp(0.0, 1.0);
            }
        }
    };

    for (i, n) in names.iter().enumerate() {
        if i >= weights.len() {
            break;
        }
        let nl = n.as_str();
        if nl.starts_with("viseme_")
            || nl.starts_with("mouth")
            || nl.starts_with("jaw")
            || nl == "tongueOut"
        {
            weights[i] = 0.0;
        }
    }
    set(weights, names, "mouthClose", 0.0);
    set(weights, names, "jawOpen", jaw);
    set(weights, names, "mouthOpen", open);

    if shape.speaking {
        set(weights, names, "mouthLowerDownLeft", shape.lower);
        set(weights, names, "mouthLowerDownRight", shape.lower);
        set(weights, names, "mouthFunnel", shape.funnel);
        set(weights, names, "mouthPucker", shape.pucker);
        if kind == MeshKind::Tongue {
            set(weights, names, "tongueOut", (jaw * 0.08).clamp(0.0, 0.12));
        }
        if let Some(vname) = shape.viseme {
            let vw = match vname {
                "viseme_sil" | "viseme_PP" => 0.45,
                "viseme_SS" | "viseme_FF" | "viseme_nn" => 0.35,
                _ => (0.22 + open * 1.5).clamp(0.15, 0.50),
            };
            set(weights, names, vname, vw);
        }
    } else {
        set(weights, names, "viseme_sil", 0.3);
    }

    if matches!(kind, MeshKind::Head | MeshKind::Other) {
        set(weights, names, "eyeBlinkLeft", shape.blink);
        set(weights, names, "eyeBlinkRight", shape.blink);
    }
}

/// Root framing only — **no** bone Euler hacks (animation owns the skeleton).
pub fn animate_body(
    time: Res<Time>,
    status: Res<FaceStatus>,
    mut root_q: Query<&mut Transform, With<FaceRoot>>,
) {
    let t = time.elapsed_secs();
    let Ok(mut xf) = root_q.get_single_mut() else {
        return;
    };
    let amp = match status.state {
        RuntimeState::Idle => 0.001,
        RuntimeState::Listening => 0.0015,
        RuntimeState::Thinking => 0.003,
        RuntimeState::Speaking => 0.002,
    };
    // Full figure: feet on ground plane in view
    xf.translation.x = (t * 0.8).sin() * amp;
    xf.translation.y = -0.05 + (t * 1.2).sin() * amp * 0.25;
    xf.translation.z = 0.0;
    xf.scale = Vec3::splat(1.0);
    xf.rotation = Quat::from_euler(
        EulerRot::YXZ,
        (t * 0.35).sin() * 0.02,
        (t * 0.45).cos() * 0.008,
        0.0,
    );
}

pub fn animate_face(
    time: Res<Time>,
    status: Res<FaceStatus>,
    mut mouth_q: Query<&mut Transform, (With<FaceMouth>, Without<FaceRoot>)>,
    parts: Res<FaceParts>,
) {
    if parts.has_morphs {
        return;
    }
    let t = time.elapsed_secs();
    let open = match status.state {
        RuntimeState::Speaking => {
            let c = ((t * 12.0).sin() * 0.5 + 0.5) * 0.4 + 0.2;
            status.mouth.max(c) * 0.15
        }
        _ => 0.02,
    };
    if let Ok(mut xf) = mouth_q.get_single_mut() {
        xf.scale = Vec3::new(1.4, 0.35 + open * 8.0, 0.85);
    }
}

fn spawn_procedural_fallback(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    parts: &mut ResMut<FaceParts>,
) {
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.93, 0.80, 0.74),
        perceptual_roughness: 0.35,
        ..default()
    });
    let lip = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.15, 0.18),
        perceptual_roughness: 0.4,
        ..default()
    });
    let root = commands
        .spawn((
            FaceRoot,
            ProceduralFace,
            Transform::from_xyz(0.0, 0.05, 0.0),
            Visibility::default(),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        let head = parent
            .spawn((
                FaceHead,
                Mesh3d(meshes.add(Sphere::new(0.48).mesh().uv(32, 20))),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 0.14, 0.0),
            ))
            .id();
        parts.head = Some(head);
        let mouth = parent
            .spawn((
                FaceMouth,
                Mesh3d(meshes.add(Sphere::new(0.06).mesh().uv(12, 8))),
                MeshMaterial3d(lip),
                Transform::from_xyz(0.0, -0.08, 0.48),
            ))
            .id();
        parts.mouth = Some(mouth);
    });
}
