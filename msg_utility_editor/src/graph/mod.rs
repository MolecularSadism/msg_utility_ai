mod convert;
mod viewer;

use bevy_inspector_egui::bevy_egui::egui;
use msg_utility_ai::scoring::ResponseCurve;
use serde::{Deserialize, Serialize};

pub use convert::{apply_layout, graph_to_profile, profile_to_graph};
pub use viewer::{EditorViewer, SELECT_STROKE_COLOR, SELECT_STROKE_WIDTH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorNode {
    Mul {
        input_count: usize,
    },
    Max {
        input_count: usize,
    },
    Average {
        input_count: usize,
    },
    Geom {
        input_count: usize,
    },
    Not,
    Weighted {
        weight: f32,
    },
    Curve {
        curve: ResponseCurve,
        #[serde(default)]
        collapsed: bool,
    },
    Clamp,
    Normalize,
    Consider {
        consideration: String,
    },
    Fixed {
        value: f32,
    },
    Composite {
        name: String,
    },
    Behavior {
        behavior: String,
        input_count: usize,
    },
}

impl EditorNode {
    pub fn title(&self) -> String {
        match self {
            EditorNode::Mul { .. } => "MUL".into(),
            EditorNode::Max { .. } => "MAX".into(),
            EditorNode::Average { .. } => "AVG".into(),
            EditorNode::Geom { .. } => "GEOM".into(),
            EditorNode::Not => "NOT".into(),
            EditorNode::Weighted { weight } => format!("× {weight:.2}"),
            EditorNode::Curve { curve, .. } => format!("Curve: {:?}", curve.curve_type),
            EditorNode::Clamp => "Clamp".into(),
            EditorNode::Normalize => "Normalize".into(),
            EditorNode::Consider { .. } => "Consider".into(),
            EditorNode::Fixed { .. } => "Fixed".into(),
            EditorNode::Composite { .. } => "Composite".into(),
            EditorNode::Behavior { .. } => "Behavior".into(),
        }
    }

    pub fn input_count(&self) -> usize {
        match self {
            EditorNode::Mul { input_count }
            | EditorNode::Max { input_count }
            | EditorNode::Average { input_count }
            | EditorNode::Geom { input_count } => *input_count,
            EditorNode::Not | EditorNode::Weighted { .. } | EditorNode::Curve { .. } => 1,
            EditorNode::Clamp | EditorNode::Normalize => 3,
            EditorNode::Composite { .. } => 1,
            EditorNode::Behavior { input_count, .. } => *input_count,
            EditorNode::Consider { .. } | EditorNode::Fixed { .. } => 0,
        }
    }

    pub fn output_count(&self) -> usize {
        match self {
            EditorNode::Composite { .. } | EditorNode::Behavior { .. } => 0,
            _ => 1,
        }
    }

    pub fn header_color(&self) -> egui::Color32 {
        match self {
            EditorNode::Mul { .. } => egui::Color32::from_rgb(30, 70, 120),
            EditorNode::Max { .. } => egui::Color32::from_rgb(25, 90, 50),
            EditorNode::Average { .. } => egui::Color32::from_rgb(120, 80, 20),
            EditorNode::Geom { .. } => egui::Color32::from_rgb(100, 50, 120),
            EditorNode::Not => egui::Color32::from_rgb(120, 30, 30),
            EditorNode::Weighted { .. } => egui::Color32::from_rgb(70, 70, 85),
            EditorNode::Curve { .. } => egui::Color32::from_rgb(30, 100, 90),
            EditorNode::Clamp => egui::Color32::from_rgb(110, 45, 45),
            EditorNode::Normalize => egui::Color32::from_rgb(35, 65, 120),
            EditorNode::Consider { .. } => egui::Color32::from_rgb(120, 110, 25),
            EditorNode::Fixed { .. } => egui::Color32::from_rgb(55, 55, 55),
            EditorNode::Composite { .. } => egui::Color32::from_rgb(25, 110, 100),
            EditorNode::Behavior { .. } => egui::Color32::from_rgb(95, 40, 120),
        }
    }
}

/// Collect all unique consideration names from Consider nodes in the graph.
pub fn collect_consideration_names(graph: &egui_snarl::Snarl<EditorNode>) -> Vec<String> {
    let mut names: Vec<String> = graph
        .nodes()
        .filter_map(|node| {
            if let EditorNode::Consider { consideration } = node {
                Some(consideration.clone())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Collect all unique composite consideration names from Composite nodes in the graph.
pub fn collect_composite_names(graph: &egui_snarl::Snarl<EditorNode>) -> Vec<String> {
    let mut names: Vec<String> = graph
        .nodes()
        .filter_map(|node| {
            if let EditorNode::Composite { name } = node {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names.dedup();
    names
}
