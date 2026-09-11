use bevy_inspector_egui::bevy_egui::egui;
use msg_utility_ai::scoring::ResponseCurve;

use crate::document::EditorDocument;
use crate::graph::{EditorNode, graph_to_profile, profile_to_graph};
use crate::io;

pub fn show(ctx: &egui::Context, doc: &mut EditorDocument) {
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                ui.set_min_width(120.0);
                if ui.button("New").clicked() {
                    *doc = EditorDocument::default();
                    ui.close();
                }
                if ui.button("Open...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("RON", &["ron"])
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    {
                        match io::load(&path) {
                            Ok(profile) => {
                                doc.graph = profile_to_graph(&profile);
                                doc.threshold = profile.threshold;
                                doc.strategy = profile.strategy;
                                doc.file_path = Some(path);
                                doc.dirty = false;
                                doc.pending_layout = 60;
                            }
                            Err(e) => {
                                eprintln!("Failed to load: {e}");
                            }
                        }
                    }
                    ui.close();
                }
                if ui.button("Save").clicked() {
                    save(doc);
                    ui.close();
                }
                if ui.button("Save As...").clicked() {
                    save_as(doc);
                    ui.close();
                }
            });

            ui.menu_button("Node", |ui| {
                ui.set_min_width(120.0);
                let pos = egui::pos2(0.0, 0.0);

                ui.label(
                    egui::RichText::new("Logic")
                        .small()
                        .color(egui::Color32::from_gray(140)),
                );
                if ui.button("MUL").clicked() {
                    doc.graph
                        .insert_node(pos, EditorNode::Mul { input_count: 2 });
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("MAX").clicked() {
                    doc.graph
                        .insert_node(pos, EditorNode::Max { input_count: 2 });
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("NOT").clicked() {
                    doc.graph.insert_node(pos, EditorNode::Not);
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("AVG").clicked() {
                    doc.graph
                        .insert_node(pos, EditorNode::Average { input_count: 2 });
                    doc.dirty = true;
                    ui.close();
                }

                ui.separator();
                ui.label(
                    egui::RichText::new("Transform")
                        .small()
                        .color(egui::Color32::from_gray(140)),
                );
                if ui.button("Weighted").clicked() {
                    doc.graph
                        .insert_node(pos, EditorNode::Weighted { weight: 1.0 });
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("Curve").clicked() {
                    doc.graph.insert_node(
                        pos,
                        EditorNode::Curve {
                            curve: ResponseCurve::default(),
                            collapsed: false,
                        },
                    );
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("Clamp").clicked() {
                    doc.graph.insert_node(pos, EditorNode::Clamp);
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("Normalize").clicked() {
                    doc.graph.insert_node(pos, EditorNode::Normalize);
                    doc.dirty = true;
                    ui.close();
                }

                ui.separator();
                ui.label(
                    egui::RichText::new("Sources")
                        .small()
                        .color(egui::Color32::from_gray(140)),
                );
                if ui.button("Consider").clicked() {
                    doc.graph.insert_node(
                        pos,
                        EditorNode::Consider {
                            consideration: "consideration_name".into(),
                        },
                    );
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("Fixed").clicked() {
                    doc.graph.insert_node(pos, EditorNode::Fixed { value: 1.0 });
                    doc.dirty = true;
                    ui.close();
                }

                ui.separator();
                ui.label(
                    egui::RichText::new("Output")
                        .small()
                        .color(egui::Color32::from_gray(140)),
                );
                if ui.button("Composite").clicked() {
                    doc.graph.insert_node(
                        pos,
                        EditorNode::Composite {
                            name: "composite_name".into(),
                        },
                    );
                    doc.dirty = true;
                    ui.close();
                }
                if ui.button("Behavior").clicked() {
                    doc.graph.insert_node(
                        pos,
                        EditorNode::Behavior {
                            behavior: "behavior".into(),
                            input_count: 1,
                        },
                    );
                    doc.dirty = true;
                    ui.close();
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("Reset Layout").clicked() {
                    doc.pending_layout = 60;
                    ui.close();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let name = doc
                    .file_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "untitled.ai.ron".into());
                let dirty_mark = if doc.dirty { " *" } else { "" };
                ui.label(format!("{name}{dirty_mark}"));
            });
        });
    });
}

fn save(doc: &mut EditorDocument) {
    if let Some(path) = doc.file_path.clone() {
        let (profile, _, _) = graph_to_profile(&doc.graph, doc.threshold, doc.strategy);
        match io::save(&profile, &path) {
            Ok(()) => doc.dirty = false,
            Err(e) => eprintln!("Failed to save: {e}"),
        }
    } else {
        save_as(doc);
    }
}

fn save_as(doc: &mut EditorDocument) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("RON", &["ron"])
        .add_filter("JSON", &["json"])
        .save_file()
    {
        let (profile, _, _) = graph_to_profile(&doc.graph, doc.threshold, doc.strategy);
        match io::save(&profile, &path) {
            Ok(()) => {
                doc.file_path = Some(path);
                doc.dirty = false;
            }
            Err(e) => eprintln!("Failed to save: {e}"),
        }
    }
}
