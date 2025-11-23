use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy_egui::EguiContexts;

pub struct ControlsPlugin;
impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_move);
    }
}

fn camera_move(
    mut q_cam: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll: EventReader<MouseWheel>,
    mut egui: EguiContexts,
    time: Res<Time>,
) {
    if egui.ctx_mut().wants_pointer_input() || egui.ctx_mut().wants_keyboard_input() {
        return;
    }

    let (mut t, mut proj) = q_cam.single_mut();

    // movement
    let f: f32 = 20.0 * time.delta_seconds();

    // Convert to Vec3 so we can modify
    let mut forward: Vec3 = t.forward().into();
    let mut right: Vec3 = t.right().into();

    // Flatten to XZ plane
    forward.y = 0.0;
    right.y = 0.0;

    if keys.pressed(KeyCode::KeyW) {
        t.translation += forward * f;
    }
    if keys.pressed(KeyCode::KeyS) {
        t.translation -= forward * f;
    }
    if keys.pressed(KeyCode::KeyA) {
        t.translation -= right * f;
    }
    if keys.pressed(KeyCode::KeyD) {
        t.translation += right * f;
    }

    // zoom
    if let Projection::Orthographic(ref mut ortho) = *proj {
        const MIN_SCALE: f32 = 0.02;
        const MAX_SCALE: f32 = 0.04;
        const ZOOM_SENSITIVITY: f32 = 0.1;

        for ev in scroll.read() {
            let zoom_factor = (1.0 - ev.y * ZOOM_SENSITIVITY).clamp(0.5, 1.5);
            ortho.scale = (ortho.scale * zoom_factor).clamp(MIN_SCALE, MAX_SCALE);
        }
    }
}
