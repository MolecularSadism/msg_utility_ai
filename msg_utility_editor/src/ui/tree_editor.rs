use bevy_inspector_egui::bevy_egui::egui;
use egui_snarl::ui::{NodeLayout, PinPlacement, SelectionStyle, SnarlStyle, WireLayer, WireStyle};

use crate::document::EditorDocument;
use crate::graph::{EditorViewer, SELECT_STROKE_COLOR, SELECT_STROKE_WIDTH, apply_layout};
use crate::simulation;

pub const DEFAULT_ZOOM: f32 = 0.5;

pub fn show(ui: &mut egui::Ui, doc: &mut EditorDocument) {
    let (sim_result, node_scores, behavior_scores) =
        simulation::run_simulation(&doc.graph, &doc.sensor_values, doc.threshold, doc.strategy);
    doc.sim_result = Some(sim_result);

    let laying_out = doc.pending_layout > 0;

    let mut viewer = EditorViewer::new();
    viewer.node_scores = node_scores;
    viewer.behavior_scores = behavior_scores;
    viewer.collect_sizes = laying_out;

    let style = SnarlStyle {
        node_layout: Some(NodeLayout::coil()),
        pin_placement: Some(PinPlacement::Edge),
        pin_size: Some(7.0),
        node_frame: Some(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(0x1c, 0x28, 0x2f))
                .inner_margin(egui::Margin {
                    left: 8i8,
                    right: 8i8,
                    top: 0i8,
                    bottom: 8i8,
                })
                .outer_margin(egui::Margin {
                    left: 0i8,
                    right: 0i8,
                    top: 0i8,
                    bottom: 0i8,
                })
                .corner_radius(egui::CornerRadius::same(8))
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE),
        ),
        header_frame: Some(
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 8i8,
                    right: 8i8,
                    top: 4i8,
                    bottom: 4i8,
                })
                .outer_margin(egui::Margin {
                    left: 0i8,
                    right: 0i8,
                    top: 0i8,
                    bottom: 0i8,
                })
                .corner_radius(egui::CornerRadius {
                    nw: 6,
                    ne: 6,
                    sw: 0,
                    se: 0,
                })
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE),
        ),
        bg_pattern: Some(egui_snarl::ui::BackgroundPattern::NoPattern),
        bg_frame: Some(
            egui::Frame::new()
                .fill(egui::Color32::from_gray(25))
                .inner_margin(egui::Margin::ZERO)
                .outer_margin(egui::Margin::ZERO)
                .corner_radius(egui::CornerRadius::ZERO)
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE),
        ),
        wire_style: Some(WireStyle::Bezier5),
        wire_width: Some(2.0),
        wire_layer: Some(WireLayer::AboveNodes),
        collapsible: Some(false),
        header_drag_space: Some(egui::vec2(0.0, 0.0)),
        min_scale: Some(DEFAULT_ZOOM * 0.2),
        max_scale: Some(DEFAULT_ZOOM * 2.0),
        select_style: Some(SelectionStyle {
            margin: egui::Margin::same(2),
            rounding: egui::CornerRadius::same(10),
            fill: egui::Color32::from_rgba_premultiplied(180, 180, 255, 15),
            stroke: egui::Stroke::new(SELECT_STROKE_WIDTH, SELECT_STROKE_COLOR),
        }),
        ..Default::default()
    };

    ui.style_mut().visuals.override_text_color = Some(egui::Color32::WHITE);
    doc.graph
        .show(&mut viewer, &style, egui::Id::new("score_graph"), ui);
    ui.style_mut().visuals.override_text_color = None;

    if laying_out && !viewer.node_sizes.is_empty() {
        apply_layout(&mut doc.graph, &viewer.node_sizes);
        doc.pending_layout -= 1;
    }
}
