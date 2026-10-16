use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{self, EguiContexts, EguiPrimaryContextPass, egui};
use egui_phosphor::add_to_fonts;

use crate::document::EditorDocument;

mod properties_panel;
mod simulation_panel;
mod toolbar;
mod tree_editor;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            EguiPrimaryContextPass,
            setup_phosphor_fonts.before(ui_system).run_if(run_once),
        );
        app.add_systems(EguiPrimaryContextPass, ui_system);
    }
}

fn setup_phosphor_fonts(mut ctx: EguiContexts) {
    if let Ok(egui_ctx) = ctx.ctx_mut() {
        let mut fonts = bevy_egui::egui::FontDefinitions::default();
        add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        egui_ctx.set_fonts(fonts);
    }
}

fn ui_system(mut contexts: EguiContexts, mut doc: ResMut<EditorDocument>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut root_ui = egui::Ui::new(
        ctx.clone(),
        egui::Id::new("editor_root"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.content_rect()),
    );

    toolbar::show(&mut root_ui, &mut doc);

    egui::Panel::right("properties_panel")
        .resizable(true)
        .default_size(280.0)
        .show_inside(&mut root_ui, |ui| {
            properties_panel::show(ui, &mut doc);
            ui.separator();
            simulation_panel::show(ui, &mut doc);
        });

    egui::CentralPanel::default().show_inside(&mut root_ui, |ui| {
        tree_editor::show(ui, &mut doc);
    });
}
