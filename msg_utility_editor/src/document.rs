use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::egui;
use egui_snarl::{InPinId, OutPinId, Snarl};
use msg_utility_ai::picker::PickStrategy;

use crate::graph::EditorNode;

/// Grouped score for all instances of a behavior with the same name.
#[derive(Debug, Clone)]
pub struct BehaviorGroupScore {
    pub name: String,
    /// Best score across all thoughts for this behavior.
    pub top_score: f32,
    /// Individual score per thought (one per Behavior input pin).
    pub thoughts: Vec<f32>,
    /// Whether this behavior group contains the winning action.
    pub is_winner: bool,
}

#[derive(Debug, Clone)]
pub struct SimResult {
    pub groups: Vec<BehaviorGroupScore>,
    pub winner: Option<String>,
}

#[derive(Resource)]
pub struct EditorDocument {
    pub graph: Snarl<EditorNode>,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub sensor_values: HashMap<String, f32>,
    pub sim_result: Option<SimResult>,
    pub threshold: f32,
    pub strategy: PickStrategy,
    /// Frame countdown for continuous layout. While > 0, layout is
    /// re-applied every frame so nodes animate into position.
    pub pending_layout: u16,
}

impl Default for EditorDocument {
    fn default() -> Self {
        Self {
            graph: default_occupation_graph(),
            file_path: None,
            dirty: false,
            sensor_values: HashMap::new(),
            sim_result: None,
            threshold: 0.1,
            strategy: PickStrategy::HighestScore,
            pending_layout: 0,
        }
    }
}

/// Creates a graph pre-populated with a default behavior:
/// `Fixed(0.5)` connected to a `Behavior("Idle")`.
fn default_occupation_graph() -> Snarl<EditorNode> {
    let mut graph = Snarl::new();

    let root_id = graph.insert_node(
        egui::pos2(500.0, 50.0),
        EditorNode::Behavior {
            behavior: "Idle".into(),
            input_count: 1,
        },
    );

    let fixed_id = graph.insert_node(egui::pos2(300.0, 200.0), EditorNode::Fixed { value: 0.5 });

    graph.connect(
        OutPinId {
            node: fixed_id,
            output: 0,
        },
        InPinId {
            node: root_id,
            input: 0,
        },
    );

    graph
}
