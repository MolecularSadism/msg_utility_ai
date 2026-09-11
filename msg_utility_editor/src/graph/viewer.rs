use std::collections::HashMap;

use bevy_inspector_egui::bevy_egui::egui;
use egui_phosphor::regular;
use egui_snarl::ui::{BackgroundPattern, PinInfo, SnarlPin, SnarlStyle};
use egui_snarl::{InPin, NodeId, OutPin, Snarl};

use msg_utility_ai::scoring::{CurveType, ResponseCurve};

use super::EditorNode;

pub const GRID_SPACING: f32 = 25.0;

const HOVER_STROKE_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 220, 100);
const HOVER_STROKE_WIDTH: f32 = 1.5;
pub const SELECT_STROKE_COLOR: egui::Color32 = egui::Color32::from_rgb(180, 180, 255);
pub const SELECT_STROKE_WIDTH: f32 = 1.5;

const NODE_MIN_WIDTH: f32 = 120.0;

const PIN_COLOR_DEFAULT: egui::Color32 = egui::Color32::from_rgb(180, 180, 220);
const PIN_COLOR_BEHAVIOR: egui::Color32 = egui::Color32::from_rgb(200, 140, 255);

const SNAP_STRENGTH: f32 = 0.15;
const SNAP_VELOCITY_THRESHOLD: f32 = 50.0;

fn snap_to_grid(pos: egui::Pos2) -> egui::Pos2 {
    egui::pos2(
        (pos.x / GRID_SPACING).round() * GRID_SPACING,
        (pos.y / GRID_SPACING).round() * GRID_SPACING,
    )
}

fn soft_snap_to_grid(pos: egui::Pos2, pointer_speed: f32) -> egui::Pos2 {
    let target = snap_to_grid(pos);
    let dist = (target - pos).length();
    if dist < 0.5 {
        return target;
    }
    let t = (1.0 - (pointer_speed / SNAP_VELOCITY_THRESHOLD).clamp(0.0, 1.0)) * SNAP_STRENGTH;
    egui::pos2(
        pos.x + (target.x - pos.x) * t,
        pos.y + (target.y - pos.y) * t,
    )
}

const CURVE_PLOT_SIZE: f32 = 120.0;
const CURVE_SAMPLES: usize = 100;
const CURVE_COLOR_START: egui::Color32 = egui::Color32::from_rgb(80, 220, 120);
const CURVE_COLOR_END: egui::Color32 = egui::Color32::from_rgb(60, 160, 240);

fn lerp_color(t: f32) -> egui::Color32 {
    CURVE_COLOR_START.lerp_to_gamma(CURVE_COLOR_END, t)
}

fn draw_curve_plot(ui: &mut egui::Ui, curve: &ResponseCurve, input_value: Option<f32>) {
    let (response, painter) = ui.allocate_painter(
        egui::vec2(CURVE_PLOT_SIZE, CURVE_PLOT_SIZE),
        egui::Sense::hover(),
    );
    let rect = response.rect;

    // Background
    painter.rect_filled(rect, 2.0, egui::Color32::from_gray(18));

    // Border
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(50)),
        egui::StrokeKind::Inside,
    );

    // Coordinate mapping: x [0,1] → left..right, y [0,1] → bottom..top
    let to_screen = |x: f32, y: f32| -> egui::Pos2 {
        egui::pos2(
            rect.left() + x * rect.width(),
            rect.bottom() - y * rect.height(),
        )
    };

    // Draw curve segments with color gradient
    let mut prev_x = 0.0f32;
    let mut prev_y = curve.sample(0.0);
    for i in 1..=CURVE_SAMPLES {
        let t = i as f32 / CURVE_SAMPLES as f32;
        let y = curve.sample(t);
        let color = lerp_color(prev_x);
        painter.line_segment(
            [to_screen(prev_x, prev_y), to_screen(t, y)],
            egui::Stroke::new(2.0, color),
        );
        prev_x = t;
        prev_y = y;
    }

    // Active value visualization
    if let Some(input) = input_value {
        let input = input.clamp(0.0, 1.0);
        let output = curve.sample(input);

        // Horizontal fill line at y = output, from curve point to x = 1.0
        let line_color = egui::Color32::from_rgba_premultiplied(255, 255, 255, 60);
        painter.line_segment(
            [to_screen(input, output), to_screen(1.0, output)],
            egui::Stroke::new(1.5, line_color),
        );

        // Dot on the curve at (input, output)
        painter.circle_filled(to_screen(input, output), 4.0, egui::Color32::WHITE);
        painter.circle_stroke(
            to_screen(input, output),
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(40)),
        );

        // Dot at x = 1.0 showing the final output
        painter.circle_filled(to_screen(1.0, output), 3.5, egui::Color32::WHITE);
        painter.circle_stroke(
            to_screen(1.0, output),
            3.5,
            egui::Stroke::new(1.0, egui::Color32::from_gray(40)),
        );
    }
}

fn input_pin(node: &EditorNode, connected_color: Option<egui::Color32>) -> PinInfo {
    match connected_color {
        Some(c) => PinInfo::circle()
            .with_fill(egui::Color32::from_gray(80))
            .with_stroke(egui::Stroke::new(2.0, c)),
        None => {
            let border = match node {
                EditorNode::Behavior { .. } => PIN_COLOR_BEHAVIOR,
                _ => PIN_COLOR_DEFAULT,
            };
            PinInfo::circle()
                .with_fill(egui::Color32::WHITE)
                .with_stroke(egui::Stroke::new(2.0, border))
        }
    }
}

fn output_pin(node: &EditorNode) -> PinInfo {
    let color = node.header_color();
    PinInfo::circle()
        .with_fill(color)
        .with_stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(140)))
}

pub struct EditorViewer {
    pub node_scores: HashMap<NodeId, f32>,
    /// Per-input scores for each Behavior node (one entry per input pin).
    pub behavior_scores: HashMap<NodeId, Vec<f32>>,
    pub collect_sizes: bool,
    pub node_sizes: HashMap<NodeId, egui::Vec2>,
}

impl EditorViewer {
    pub fn new() -> Self {
        Self {
            node_scores: HashMap::new(),
            behavior_scores: HashMap::new(),
            collect_sizes: false,
            node_sizes: HashMap::new(),
        }
    }
}

impl egui_snarl::ui::SnarlViewer<EditorNode> for EditorViewer {
    fn title(&mut self, node: &EditorNode) -> String {
        match node {
            EditorNode::Consider { .. }
            | EditorNode::Composite { .. }
            | EditorNode::Behavior { .. } => String::new(),
            _ => node.title(),
        }
    }

    fn show_header(
        &mut self,
        node_id: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let node = &mut snarl[node_id];
        match node {
            EditorNode::Consider { consideration } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(regular::DOWNLOAD_SIMPLE);
                    ui.add(
                        egui::TextEdit::singleline(consideration)
                            .desired_width(NODE_MIN_WIDTH)
                            .clip_text(false),
                    );
                });
            }
            EditorNode::Composite { name } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(regular::BROADCAST);
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .desired_width(NODE_MIN_WIDTH)
                            .clip_text(false),
                    );
                });
            }
            EditorNode::Behavior { behavior, .. } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(regular::HEAD_CIRCUIT);
                    ui.add(
                        egui::TextEdit::singleline(behavior)
                            .desired_width(NODE_MIN_WIDTH)
                            .clip_text(false),
                    );
                });
            }
            _ => {
                ui.allocate_ui_with_layout(
                    egui::vec2(NODE_MIN_WIDTH, 0.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(node.title());
                    },
                );
            }
        }
    }

    fn inputs(&mut self, node: &EditorNode) -> usize {
        node.input_count()
    }

    fn outputs(&mut self, node: &EditorNode) -> usize {
        node.output_count()
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) -> impl SnarlPin + 'static {
        let connected_color = pin.remotes.first().map(|remote| {
            snarl
                .get_node(remote.node)
                .map_or(PIN_COLOR_DEFAULT, |n| n.header_color())
        });

        let node = &snarl[pin.id.node];
        ui.spacing_mut().item_spacing.x = 2.0;
        match node {
            EditorNode::Clamp | EditorNode::Normalize => {
                let label = match pin.id.input {
                    0 => "value",
                    1 => "min",
                    2 => "max",
                    _ => "",
                };
                ui.label(label);
            }
            EditorNode::Mul { .. }
            | EditorNode::Max { .. }
            | EditorNode::Average { .. }
            | EditorNode::Geom { .. }
            | EditorNode::Behavior { .. } => {
                ui.label(
                    egui::RichText::new(format!("{}", pin.id.input))
                        .small()
                        .color(egui::Color32::from_gray(140)),
                );
            }
            _ => {
                ui.allocate_space(egui::vec2(0.0, 0.0));
            }
        }
        input_pin(node, connected_color)
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) -> impl SnarlPin + 'static {
        let node = &snarl[pin.id.node];
        ui.spacing_mut().item_spacing.x = 2.0;
        if let Some(&score) = self.node_scores.get(&pin.id.node) {
            ui.label(
                egui::RichText::new(format!("{score:.2}"))
                    .small()
                    .color(egui::Color32::from_gray(200)),
            );
        }
        output_pin(node)
    }

    fn has_body(&mut self, node: &EditorNode) -> bool {
        !matches!(
            node,
            EditorNode::Not | EditorNode::Clamp | EditorNode::Normalize
        )
    }

    fn show_body(
        &mut self,
        node_id: NodeId,
        inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) {
        // Check link status before mutable borrow.
        let link_info: Option<(bool, usize)> = match &snarl[node_id] {
            EditorNode::Consider { consideration } => {
                let has_composite = snarl
                    .nodes()
                    .any(|n| matches!(n, EditorNode::Composite { name } if name == consideration));
                Some((has_composite, 0))
            }
            EditorNode::Composite { name } => {
                let count = snarl
                    .nodes()
                    .filter(|n| matches!(n, EditorNode::Consider { consideration } if consideration == name))
                    .count();
                Some((count > 0, count))
            }
            _ => None,
        };
        let is_composed = link_info.is_some_and(|(linked, _)| linked);
        let linked_count = link_info.map_or(0, |(_, c)| c);

        let node = &mut snarl[node_id];
        match node {
            EditorNode::Weighted { weight } => {
                ui.add(
                    egui::DragValue::new(weight)
                        .prefix("w: ")
                        .speed(0.01)
                        .range(0.0..=10.0)
                        .fixed_decimals(2),
                );
            }
            EditorNode::Fixed { value } => {
                ui.add(
                    egui::DragValue::new(value)
                        .speed(0.01)
                        .range(0.0..=1.0)
                        .fixed_decimals(2),
                );
            }
            EditorNode::Consider { .. } => {
                if is_composed {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(
                            egui::RichText::new(regular::WIFI_HIGH)
                                .color(egui::Color32::from_rgb(80, 200, 120)),
                        );
                        ui.label(
                            egui::RichText::new("linked")
                                .small()
                                .color(egui::Color32::from_gray(180)),
                        );
                    });
                }
            }
            EditorNode::Composite { .. } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if is_composed {
                        ui.label(
                            egui::RichText::new(regular::WIFI_HIGH)
                                .color(egui::Color32::from_rgb(80, 200, 120)),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{linked_count} consideration{} linked",
                                if linked_count == 1 { "" } else { "s" }
                            ))
                            .small()
                            .color(egui::Color32::from_gray(180)),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(regular::WIFI_X)
                                .color(egui::Color32::from_rgb(220, 60, 60)),
                        );
                        ui.label(
                            egui::RichText::new("link by name")
                                .small()
                                .color(egui::Color32::from_gray(180)),
                        );
                    }
                });
            }
            EditorNode::Behavior { input_count, .. } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if ui
                        .add(
                            egui::Button::new("+")
                                .small()
                                .min_size(egui::vec2(20.0, 18.0)),
                        )
                        .clicked()
                    {
                        *input_count += 1;
                    }
                    if *input_count > 1
                        && ui
                            .add(
                                egui::Button::new("\u{2212}")
                                    .small()
                                    .min_size(egui::vec2(20.0, 18.0)),
                            )
                            .clicked()
                    {
                        *input_count -= 1;
                    }
                    ui.label(
                        egui::RichText::new(format!("{} inputs", *input_count))
                            .small()
                            .color(egui::Color32::from_gray(160)),
                    );
                });
                // Per-thought scores
                if let Some(scores) = self.behavior_scores.get(&node_id) {
                    for (i, &score) in scores.iter().enumerate() {
                        ui.label(
                            egui::RichText::new(format!("  {i}: {score:.2}"))
                                .small()
                                .color(egui::Color32::from_gray(180)),
                        );
                    }
                }
            }
            EditorNode::Curve { curve, collapsed } => {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(node_id)
                            .selected_text(format!("{:?}", curve.curve_type))
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for variant in CurveType::ALL {
                                    ui.selectable_value(
                                        &mut curve.curve_type,
                                        *variant,
                                        format!("{variant:?}"),
                                    );
                                }
                            });
                        let icon = if *collapsed {
                            regular::CARET_DOWN
                        } else {
                            regular::CARET_UP
                        };
                        if ui.add(egui::Button::new(icon).frame(false)).clicked() {
                            *collapsed = !*collapsed;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(
                            egui::RichText::new("params")
                                .small()
                                .color(egui::Color32::from_gray(140)),
                        );
                        ui.add(
                            egui::DragValue::new(&mut curve.m)
                                .speed(0.01)
                                .fixed_decimals(2)
                                .range(-10.0..=10.0)
                                .max_decimals(2)
                                .update_while_editing(false),
                        )
                        .on_hover_text("m: slope / multiplier");
                        ui.add(
                            egui::DragValue::new(&mut curve.k)
                                .speed(0.01)
                                .fixed_decimals(2)
                                .range(-20.0..=20.0)
                                .max_decimals(2)
                                .update_while_editing(false),
                        )
                        .on_hover_text("k: exponent / steepness");
                        ui.add(
                            egui::DragValue::new(&mut curve.b)
                                .speed(0.01)
                                .fixed_decimals(2)
                                .range(-1.0..=1.0)
                                .max_decimals(2)
                                .update_while_editing(false),
                        )
                        .on_hover_text("b: vertical shift");
                        ui.add(
                            egui::DragValue::new(&mut curve.c)
                                .speed(0.01)
                                .fixed_decimals(2)
                                .range(-1.0..=1.0)
                                .max_decimals(2)
                                .update_while_editing(false),
                        )
                        .on_hover_text("c: horizontal shift");
                    });
                    if !*collapsed {
                        let input_value = inputs
                            .first()
                            .and_then(|pin| pin.remotes.first())
                            .and_then(|remote| self.node_scores.get(&remote.node))
                            .copied();
                        draw_curve_plot(ui, curve, input_value);
                    }
                });
            }
            EditorNode::Clamp | EditorNode::Normalize => {}
            EditorNode::Mul { input_count }
            | EditorNode::Max { input_count }
            | EditorNode::Average { input_count }
            | EditorNode::Geom { input_count } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if ui
                        .add(
                            egui::Button::new("+")
                                .small()
                                .min_size(egui::vec2(20.0, 18.0)),
                        )
                        .clicked()
                    {
                        *input_count += 1;
                    }
                    if *input_count > 1
                        && ui
                            .add(
                                egui::Button::new("\u{2212}")
                                    .small()
                                    .min_size(egui::vec2(20.0, 18.0)),
                            )
                            .clicked()
                    {
                        *input_count -= 1;
                    }
                    ui.label(
                        egui::RichText::new(format!("{} inputs", *input_count))
                            .small()
                            .color(egui::Color32::from_gray(160)),
                    );
                });
            }
            EditorNode::Not => {}
        }
    }

    fn draw_background(
        &mut self,
        _background: Option<&BackgroundPattern>,
        viewport: &egui::Rect,
        _snarl_style: &SnarlStyle,
        _style: &egui::Style,
        painter: &egui::Painter,
        _snarl: &Snarl<EditorNode>,
    ) {
        let dot_color = egui::Color32::from_rgba_premultiplied(80, 80, 80, 60);
        let dot_radius = 1.0;

        let min_x = (viewport.min.x / GRID_SPACING).floor() as i64;
        let max_x = (viewport.max.x / GRID_SPACING).ceil() as i64;
        let min_y = (viewport.min.y / GRID_SPACING).floor() as i64;
        let max_y = (viewport.max.y / GRID_SPACING).ceil() as i64;

        for gx in min_x..=max_x {
            for gy in min_y..=max_y {
                let x = gx as f32 * GRID_SPACING;
                let y = gy as f32 * GRID_SPACING;
                painter.circle_filled(egui::pos2(x, y), dot_radius, dot_color);
            }
        }
    }

    fn final_node_rect(
        &mut self,
        node: NodeId,
        rect: egui::Rect,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) {
        if self.collect_sizes {
            self.node_sizes.insert(node, rect.size());
        }

        let speed = ui.input(|i| i.pointer.velocity().length());
        if let Some(info) = snarl.get_node_info_mut(node) {
            info.pos = soft_snap_to_grid(info.pos, speed);
        }

        let screen_rect = if let Some(to_global) = ui.ctx().layer_transform_to_global(ui.layer_id())
        {
            to_global * rect
        } else {
            rect
        };
        let hovered = ui
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|pos| screen_rect.contains(pos));

        if hovered {
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(8),
                egui::Stroke::new(HOVER_STROKE_WIDTH, HOVER_STROKE_COLOR),
                egui::StrokeKind::Outside,
            );
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<EditorNode>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let snapped = snap_to_grid(pos);

        ui.set_min_width(120.0);
        ui.label(egui::RichText::new("Add Node").strong());
        ui.separator();

        ui.label(
            egui::RichText::new("Logic")
                .small()
                .color(egui::Color32::from_gray(140)),
        );
        if ui.button("MUL").clicked() {
            snarl.insert_node(snapped, EditorNode::Mul { input_count: 2 });
            ui.close();
        }
        if ui.button("MAX").clicked() {
            snarl.insert_node(snapped, EditorNode::Max { input_count: 2 });
            ui.close();
        }
        if ui.button("NOT").clicked() {
            snarl.insert_node(snapped, EditorNode::Not);
            ui.close();
        }
        if ui.button("AVG").clicked() {
            snarl.insert_node(snapped, EditorNode::Average { input_count: 2 });
            ui.close();
        }
        if ui.button("GEOM").clicked() {
            snarl.insert_node(snapped, EditorNode::Geom { input_count: 2 });
            ui.close();
        }

        ui.separator();
        ui.label(
            egui::RichText::new("Transform")
                .small()
                .color(egui::Color32::from_gray(140)),
        );
        if ui.button("Weighted").clicked() {
            snarl.insert_node(snapped, EditorNode::Weighted { weight: 1.0 });
            ui.close();
        }
        if ui.button("Curve").clicked() {
            snarl.insert_node(
                snapped,
                EditorNode::Curve {
                    curve: ResponseCurve::default(),
                    collapsed: false,
                },
            );
            ui.close();
        }
        if ui.button("Clamp").clicked() {
            snarl.insert_node(snapped, EditorNode::Clamp);
            ui.close();
        }
        if ui.button("Normalize").clicked() {
            snarl.insert_node(snapped, EditorNode::Normalize);
            ui.close();
        }

        ui.separator();
        ui.label(
            egui::RichText::new("Sources")
                .small()
                .color(egui::Color32::from_gray(140)),
        );
        if ui.button("Consider").clicked() {
            snarl.insert_node(
                snapped,
                EditorNode::Consider {
                    consideration: "consideration_name".into(),
                },
            );
            ui.close();
        }
        if ui.button("Fixed").clicked() {
            snarl.insert_node(snapped, EditorNode::Fixed { value: 1.0 });
            ui.close();
        }

        ui.separator();
        ui.label(
            egui::RichText::new("Output")
                .small()
                .color(egui::Color32::from_gray(140)),
        );
        if ui.button("Composite").clicked() {
            snarl.insert_node(
                snapped,
                EditorNode::Composite {
                    name: "composite_name".into(),
                },
            );
            ui.close();
        }
        if ui.button("Behavior").clicked() {
            snarl.insert_node(
                snapped,
                EditorNode::Behavior {
                    behavior: "behavior".into(),
                    input_count: 1,
                },
            );
            ui.close();
        }
    }

    fn has_node_menu(&mut self, _node: &EditorNode) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        node_id: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) {
        if ui.button("Delete").clicked() {
            snarl.remove_node(node_id);
            ui.close();
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<EditorNode>) {
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }
        snarl.connect(from.id, to.id);
    }

    fn header_frame(
        &mut self,
        _default: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        snarl: &Snarl<EditorNode>,
    ) -> egui::Frame {
        let bg_color = snarl[node].header_color();
        egui::Frame::new()
            .fill(bg_color)
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
                bottom: 2i8,
            })
            .corner_radius(egui::CornerRadius {
                nw: 6,
                ne: 6,
                sw: 0,
                se: 0,
            })
            .stroke(egui::Stroke::NONE)
    }

    fn has_on_hover_popup(&mut self, _: &EditorNode) -> bool {
        true
    }

    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<EditorNode>,
    ) {
        let desc = match &snarl[node] {
            EditorNode::Mul { .. } => "Output = product(inputs). Any zero kills the score.",
            EditorNode::Max { .. } => "Output = max(inputs).",
            EditorNode::Average { .. } => "Output = average(inputs).",
            EditorNode::Geom { .. } => "Output = geometric mean of inputs.",
            EditorNode::Not => "Output = 1 - input. Inverts the signal.",
            EditorNode::Weighted { .. } => "Multiplies input by weight.",
            EditorNode::Curve { .. } => "Applies an easing curve to the input.",
            EditorNode::Clamp => "Clamps value between min and max.",
            EditorNode::Normalize => "Maps value from [min, max] to [0, 1].",
            EditorNode::Consider { .. } => "Reads a consideration value from the environment.",
            EditorNode::Fixed { .. } => "Outputs a constant value.",
            EditorNode::Composite { .. } => {
                "Computes a named consideration from a score tree. Use a Consider node with the same name to read the result."
            }
            EditorNode::Behavior { .. } => "Terminal node. Drives a behavior action.",
        };
        ui.label(desc);
    }
}
