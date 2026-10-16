use bevy_inspector_egui::bevy_egui::egui;
use msg_utility_ai::picker::PickStrategy;

use crate::document::EditorDocument;

fn pick_strategy_label(strategy: PickStrategy) -> &'static str {
    match strategy {
        PickStrategy::FirstToScore => "First to Score",
        PickStrategy::HighestScore => "Highest Score",
    }
}

pub fn show(ui: &mut egui::Ui, doc: &mut EditorDocument) {
    // Active action frame at the top
    if let Some(sim) = &doc.sim_result {
        let action_name = sim.winner.as_deref().unwrap_or("none");
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(40, 60, 40))
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(100, 180, 100),
            ))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Active Action")
                            .small()
                            .color(egui::Color32::from_rgb(160, 200, 160)),
                    );
                    ui.label(
                        egui::RichText::new(action_name)
                            .heading()
                            .color(egui::Color32::from_rgb(200, 255, 200)),
                    );
                });
            });
        ui.add_space(4.0);
    }

    ui.separator();
    ui.heading("Profile Settings");
    egui::Grid::new("profile_settings_grid")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Strategy:");
            ui.centered_and_justified(|ui| {
                egui::ComboBox::from_id_salt("pick_strategy")
                    .selected_text(pick_strategy_label(doc.strategy))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut doc.strategy,
                            PickStrategy::FirstToScore,
                            pick_strategy_label(PickStrategy::FirstToScore),
                        );
                        ui.selectable_value(
                            &mut doc.strategy,
                            PickStrategy::HighestScore,
                            pick_strategy_label(PickStrategy::HighestScore),
                        );
                    });
            });
            ui.end_row();

            ui.label("Threshold:");
            ui.centered_and_justified(|ui| {
                ui.add(
                    egui::DragValue::new(&mut doc.threshold)
                        .speed(0.01)
                        .range(0.0..=1.0),
                );
            });
            ui.end_row();
        });
    ui.separator();

    if let Some(sim) = &doc.sim_result {
        ui.heading("Behavior Scores");

        for group in &sim.groups {
            let prefix = if group.is_winner { "★ " } else { "  " };

            // Top-level row: behavior name with aggregated top score.
            ui.horizontal(|ui| {
                let label = egui::RichText::new(format!("{prefix}{}", group.name));
                ui.label(label);
                ui.add(
                    egui::ProgressBar::new(group.top_score)
                        .text(format!("{:.3}", group.top_score))
                        .desired_width(ui.available_width()),
                );
            });

            // If multiple thoughts, show indented sub-items.
            if group.thoughts.len() > 1 {
                for (i, &score) in group.thoughts.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        let text = egui::RichText::new(format!("Thought {}", i + 1))
                            .small()
                            .weak();
                        ui.label(text);
                        ui.add(
                            egui::ProgressBar::new(score)
                                .text(format!("{score:.3}"))
                                .desired_width(ui.available_width()),
                        );
                    });
                }
            }
        }
    }
}
