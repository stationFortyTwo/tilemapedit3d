use bevy::prelude::*;

use crate::{editor::EditorState, types::TILE_SIZE};

const GRID_COLOR: Color = Color::srgb(0.85, 0.85, 0.85);

pub fn draw_grid(mut gizmos: Gizmos, state: Res<EditorState>) {
    if !state.show_grid {
        return;
    }

    let radius_x = state.map.width as i32;
    let radius_z = state.map.height as i32;

    let cell = TILE_SIZE;
    let half_step = cell * 0.5;

    for x in -radius_x..=radius_x {
        let position = x as f32 * cell;
        gizmos.line(
            Vec3::new(position, 0.0, -radius_z as f32 * cell - half_step),
            Vec3::new(position, 0.0, radius_z as f32 * cell + half_step),
            GRID_COLOR,
        );
    }

    for z in -radius_z..=radius_z {
        let position = z as f32 * cell;
        gizmos.line(
            Vec3::new(-radius_x as f32 * cell - half_step, 0.0, position),
            Vec3::new(radius_x as f32 * cell + half_step, 0.0, position),
            GRID_COLOR,
        );
    }
}
