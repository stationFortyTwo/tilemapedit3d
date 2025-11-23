use crate::editor::{EditorTool, ExportStatus};
use crate::export;
use crate::io::{load_map, save_map};
use crate::runtime::RuntimeSplatMap;
use crate::terrain::TerrainMeshSet;
use crate::types::*;
use bevy::prelude::*;
use bevy::render::texture::Image;
use bevy::tasks::{IoTaskPool, block_on};
use bevy_egui::{EguiContexts, egui};
use rfd::AsyncFileDialog;
use std::path::{Path, PathBuf};

use crate::texture::registry::TerrainTextureRegistry;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, ui_panel.before(TerrainMeshSet::Rebuild));
    }
}

fn ui_panel(
    mut egui_ctx: EguiContexts,
    mut state: ResMut<crate::editor::EditorState>,
    textures: Res<TerrainTextureRegistry>,
    runtime_splat: Option<Res<RuntimeSplatMap>>,
    images: Res<Assets<Image>>,
) {
    let palette_items: Vec<_> = textures
        .iter()
        .map(|entry| PaletteItem {
            tile_type: entry.tile_type,
            name: entry.name.clone(),
            texture: egui_ctx.add_image(entry.preview.clone_weak()),
        })
        .collect();

    if palette_items
        .iter()
        .all(|item| item.tile_type != state.current_texture)
    {
        if let Some(first) = palette_items.first() {
            state.current_texture = first.tile_type;
        }
    }

    egui::TopBottomPanel::top("toolbar").show(egui_ctx.ctx_mut(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            ui.selectable_value(&mut state.current_tool, EditorTool::Paint, "Paint");
            ui.selectable_value(
                &mut state.current_tool,
                EditorTool::RotateRamp,
                "Rotate Ramp",
            );

            if state.current_tool == EditorTool::Paint {
                ui.separator();
                ui.label("Tile:");
                ui.selectable_value(&mut state.current_kind, TileKind::Floor, "Floor");
                ui.selectable_value(&mut state.current_kind, TileKind::Ramp, "Ramp");
            }

            ui.separator();
            ui.label("Elevation:");
            for e in 0..=3 {
                ui.selectable_value(&mut state.current_elev, e, format!("{e}"));
            }

            ui.separator();
            if ui.button("Save…").clicked() && state.save_dialog_task.is_none() {
                let mut dialog = AsyncFileDialog::new().set_title("Save Map");
                if let Some(path) = state.current_file_path.as_ref() {
                    if let Some(parent) = path.parent() {
                        dialog = dialog.set_directory(parent);
                    }
                    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                        dialog = dialog.set_file_name(file_name);
                    }
                }

                state.save_dialog_task = Some(IoTaskPool::get().spawn(async move {
                    dialog
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                }));
            }
            if ui.button("Export…").clicked()
                && state.export_dialog_task.is_none()
                && state.export_task.is_none()
            {
                let mut dialog = AsyncFileDialog::new().set_title("Export Map");
                dialog = dialog.add_filter("Tile Map Package", &["tmemapdata"]);
                if let Some(path) = state.current_file_path.as_ref() {
                    if let Some(parent) = path.parent() {
                        dialog = dialog.set_directory(parent);
                    }
                    if let Some(stem) = path.file_stem().and_then(|name| name.to_str()) {
                        dialog = dialog.set_file_name(format!("{stem}.tmemapdata"));
                    }
                } else {
                    dialog = dialog.set_file_name("map.tmemapdata");
                }

                state.export_dialog_task = Some(IoTaskPool::get().spawn(async move {
                    dialog
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                }));
            }
            if ui.button("Load…").clicked() && state.load_dialog_task.is_none() {
                let mut dialog = AsyncFileDialog::new().set_title("Open Map");
                if let Some(path) = state.current_file_path.as_ref() {
                    if let Some(parent) = path.parent() {
                        dialog = dialog.set_directory(parent);
                    }
                }

                state.load_dialog_task = Some(IoTaskPool::get().spawn(async move {
                    dialog
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                }));
            }

            ui.separator();
            ui.checkbox(&mut state.show_grid, "Gridlines");
        });

        if !palette_items.is_empty() {
            ui.separator();
            ui.collapsing("Textures", |ui| {
                const COLUMNS: usize = 4;
                let grid = egui::Grid::new("texture_palette_grid")
                    .spacing([6.0, 6.0])
                    .num_columns(COLUMNS);

                let button_outer_size = egui::Vec2::splat(36.0);
                let button_inner_size = egui::vec2(32.0, 32.0);

                grid.show(ui, |grid_ui| {
                    for (index, item) in palette_items.iter().enumerate() {
                        let is_selected = state.current_texture == item.tile_type;
                        let stroke = if is_selected {
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 122, 204))
                        } else {
                            egui::Stroke::NONE
                        };

                        let response = egui::Frame::none()
                            .inner_margin(egui::Margin::same(2.0))
                            .stroke(stroke)
                            .show(grid_ui, |ui| {
                                ui.set_min_size(button_outer_size);
                                ui.set_max_size(button_outer_size);
                                ui.centered_and_justified(|ui| {
                                    ui.add(
                                        egui::ImageButton::new(egui::load::SizedTexture {
                                            id: item.texture,
                                            size: button_inner_size,
                                        })
                                        .frame(false),
                                    )
                                })
                                .inner
                            })
                            .inner;

                        let response = response.on_hover_text(item.name.clone());

                        if response.clicked() {
                            state.current_texture = item.tile_type;
                        }

                        if index % COLUMNS == COLUMNS - 1 {
                            grid_ui.end_row();
                        }
                    }

                    if palette_items.len() % COLUMNS != 0 {
                        grid_ui.end_row();
                    }
                });
            });
        }
        if let Some(path) = state.current_file_path.as_ref() {
            ui.separator();
            ui.label(format!("Current map: {}", path.display()));
        }

        if let Some(status) = state.last_export_status.as_ref() {
            ui.separator();
            match status {
                ExportStatus::Success(message) => {
                    ui.colored_label(egui::Color32::from_rgb(56, 142, 60), message);
                }
                ExportStatus::Failure(message) => {
                    ui.colored_label(egui::Color32::from_rgb(198, 40, 40), message);
                }
            }
        }
    });

    if let Some(task) = state.save_dialog_task.as_mut() {
        if task.is_finished() {
            if let Some(path) = block_on(state.save_dialog_task.take().unwrap()) {
                if let Err(err) = save_map(&path, &state.map) {
                    eprintln!("Failed to save map: {err:?}");
                } else {
                    state.current_file_path = Some(path);
                }
            }
        }
    }

    if let Some(task) = state.export_dialog_task.as_mut() {
        if task.is_finished() {
            if let Some(path) = block_on(state.export_dialog_task.take().unwrap()) {
                let export_path = ensure_extension(path, "tmemapdata");
                match export::collect_texture_descriptors(&state.map, textures.as_ref()) {
                    Ok((descriptors, wall_descriptor)) => {
                        let map_clone = state.map.clone();
                        let export_name = infer_export_name(&state, &export_path);
                        let export_path_clone = export_path.clone();
                        let splat_png_result = if let Some(runtime) = runtime_splat.as_ref() {
                            if let Some(image) = images.get(&runtime.handle) {
                                export::encode_splatmap_png(image)
                            } else {
                                export::build_map_splatmap_png(&map_clone)
                            }
                        } else {
                            export::build_map_splatmap_png(&map_clone)
                        };

                        match splat_png_result {
                            Ok(splat_png) => {
                                state.last_export_status = None;
                                state.export_task = Some(IoTaskPool::get().spawn(async move {
                                    export::export_package(
                                        &export_path_clone,
                                        map_clone,
                                        export_name,
                                        descriptors,
                                        wall_descriptor,
                                        splat_png,
                                    )
                                    .map(|_| export_path_clone)
                                }));
                            }
                            Err(err) => {
                                eprintln!("Failed to prepare splatmap for export: {err:?}");
                                state.last_export_status =
                                    Some(ExportStatus::Failure(format!("Export failed: {err}")));
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Failed to gather textures for export: {err:?}");
                        state.last_export_status =
                            Some(ExportStatus::Failure(format!("Export failed: {err}")));
                    }
                }
            }
        }
    }

    if let Some(task) = state.export_task.as_mut() {
        if task.is_finished() {
            match block_on(state.export_task.take().unwrap()) {
                Ok(path) => {
                    state.last_export_status = Some(ExportStatus::Success(format!(
                        "Exported map to {}",
                        path.display()
                    )));
                }
                Err(err) => {
                    eprintln!("Failed to export map: {err:?}");
                    state.last_export_status =
                        Some(ExportStatus::Failure(format!("Export failed: {err}")));
                }
            }
        }
    }

    if let Some(task) = state.load_dialog_task.as_mut() {
        if task.is_finished() {
            if let Some(path) = block_on(state.load_dialog_task.take().unwrap()) {
                match load_map(&path) {
                    Ok(m) => {
                        state.map = m;
                        state.map_dirty = true;
                        state.current_file_path = Some(path);
                    }
                    Err(err) => {
                        eprintln!("Failed to load map: {err:?}");
                    }
                }
            }
        }
    }
}

struct PaletteItem {
    tile_type: TileType,
    name: String,
    texture: egui::TextureId,
}

fn ensure_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    let needs_extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| !ext.eq_ignore_ascii_case(extension))
        .unwrap_or(true);
    if needs_extension {
        path.set_extension(extension);
    }
    path
}

fn infer_export_name(state: &crate::editor::EditorState, export_path: &Path) -> String {
    if let Some(path) = state.current_file_path.as_ref() {
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            return stem.to_string();
        }
    }

    export_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.to_string())
        .unwrap_or_else(|| "Tile Map".to_string())
}
