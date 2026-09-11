use bevy_inspector_egui::bevy_egui::egui;

use crate::document::EditorDocument;
use crate::graph::{collect_composite_names, collect_consideration_names};

pub fn show(ui: &mut egui::Ui, doc: &mut EditorDocument) {
    ui.heading("Simulation");

    let consideration_names = collect_consideration_names(&doc.graph);

    if consideration_names.is_empty() {
        ui.label("No considerations in graph. Add Consider nodes to simulate.");
        return;
    }

    let composite_names = collect_composite_names(&doc.graph);

    egui::Grid::new("sensor_values_grid")
        .num_columns(2)
        .show(ui, |ui| {
            for name in &consideration_names {
                if composite_names.contains(name) {
                    continue;
                }
                let value = doc.sensor_values.entry(name.clone()).or_insert(0.0);
                ui.label(name);
                ui.add(egui::Slider::new(value, 0.0..=1.0).clamping(egui::SliderClamping::Edits));
                ui.end_row();
            }
        });

    if ui.button("Reset All").clicked() {
        for value in doc.sensor_values.values_mut() {
            *value = 0.0;
        }
    }
}
