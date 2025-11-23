use bevy::prelude::*;

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

// spawn a camera looking down on the XZ plane
fn spawn_camera(mut commands: Commands) {
    // Spawn orthographic isometric camera
    let mut transform = Transform::from_xyz(20.0, 20.0, 20.0);
    transform.rotation = Quat::from_rotation_y(45_f32.to_radians())
        * Quat::from_rotation_x(-35.264_f32.to_radians());
    const MIN_SCALE: f32 = 0.02;

    commands.spawn((Camera3dBundle {
        transform,
        projection: Projection::Orthographic(OrthographicProjection {
            scale: MIN_SCALE,
            near: -500.0,
            far: 500.0,
            ..default()
        }),
        ..default()
    },));
}
