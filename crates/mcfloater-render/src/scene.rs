use bevy::prelude::*;

/// Full-body framing — head to feet in view.
pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Pull back so feet fit; look at mid-torso.
    // Msaa::Sample4 is the Bevy 0.15 camera default (component, not a global resource).
    commands.spawn((
        Camera3d::default(),
        Msaa::Sample4,
        Transform::from_xyz(0.0, 0.95, 3.4).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    ));

    // Key
    commands.spawn((
        PointLight {
            intensity: 1_200_000.0,
            range: 18.0,
            shadows_enabled: false,
            color: Color::srgb(1.0, 0.97, 0.94),
            ..default()
        },
        Transform::from_xyz(1.4, 2.2, 2.8),
    ));

    // Fill
    commands.spawn((
        PointLight {
            intensity: 400_000.0,
            range: 14.0,
            color: Color::srgb(0.75, 0.88, 1.0),
            ..default()
        },
        Transform::from_xyz(-1.8, 1.2, 2.0),
    ));

    // Rim
    commands.spawn((
        PointLight {
            intensity: 550_000.0,
            range: 14.0,
            color: Color::srgb(0.55, 0.95, 1.0),
            ..default()
        },
        Transform::from_xyz(0.3, 2.0, -1.5),
    ));

    // Soft magenta kick
    commands.spawn((
        PointLight {
            intensity: 80_000.0,
            range: 10.0,
            color: Color::srgb(1.0, 0.30, 0.55),
            ..default()
        },
        Transform::from_xyz(-0.6, 0.4, 1.5),
    ));

    // Ground disc (helps read feet)
    let floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.07, 0.09),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(2.2))),
        MeshMaterial3d(floor),
        Transform::from_xyz(0.0, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    // CRT void backdrop
    let bezel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.035, 0.05),
        perceptual_roughness: 0.95,
        metallic: 0.05,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(5.0, 4.0, 0.12))),
        MeshMaterial3d(bezel),
        Transform::from_xyz(0.0, 1.0, -1.4),
    ));

    let line_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 0.90, 1.0, 0.04),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for i in 0..36 {
        let y = -0.4 + i as f32 * 0.09;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(4.5, 0.008, 0.015))),
            MeshMaterial3d(line_mat.clone()),
            Transform::from_xyz(0.0, y, -1.32),
        ));
    }
}
