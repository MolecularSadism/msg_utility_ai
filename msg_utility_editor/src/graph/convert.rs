use std::collections::HashMap;

use bevy_inspector_egui::bevy_egui::egui;
use egui_snarl::{InPinId, NodeId, OutPinId, Snarl};
use msg_utility_ai::picker::PickStrategy;
use msg_utility_ai::{
    ActionId, BehaviorDef, CompositeConsiderationDef, ConsiderationId, ScoreNode, UtilityAIProfile,
};

use super::EditorNode;

/// Maps each editor `NodeId` to the `ScoreNode` it produces.
///
/// Used to display per-node scores in the editor UI.
pub type NodeScoreMap = HashMap<NodeId, ScoreNode>;

/// Tracks which profile `BehaviorDef` originated from which graph node.
pub struct BehaviorOrigin {
    pub node_id: NodeId,
}

/// Convert the editor graph into a flat `UtilityAIProfile`.
///
/// Each input pin on a `Behavior` node becomes its own `BehaviorDef`.
/// The caller can use the returned `BehaviorOrigin` list to map
/// evaluated scores back to specific node inputs.
pub fn graph_to_profile(
    graph: &Snarl<EditorNode>,
    threshold: f32,
    strategy: PickStrategy,
) -> (UtilityAIProfile, NodeScoreMap, Vec<BehaviorOrigin>) {
    let mut behaviors = Vec::new();
    let mut composites = Vec::new();
    let mut node_map = NodeScoreMap::new();
    let mut origins = Vec::new();

    for (node_id, node) in graph.node_ids() {
        match node {
            EditorNode::Behavior {
                behavior,
                input_count,
            } => {
                for i in 0..*input_count {
                    let score = trace_score_tree(graph, node_id, i, &mut node_map);
                    behaviors.push(BehaviorDef {
                        action: ActionId::new(behavior),
                        score: score.clone(),
                    });
                    origins.push(BehaviorOrigin { node_id });
                }
            }
            EditorNode::Composite { name } => {
                let score = trace_score_tree(graph, node_id, 0, &mut node_map);
                composites.push(CompositeConsiderationDef {
                    id: ConsiderationId::new(name),
                    score: score.clone(),
                });
                node_map.insert(node_id, score);
            }
            _ => {}
        }
    }

    let profile = UtilityAIProfile {
        threshold,
        strategy,
        behaviors,
        composites,
    };
    (profile, node_map, origins)
}

fn trace_score_tree(
    graph: &Snarl<EditorNode>,
    node_id: NodeId,
    input_idx: usize,
    node_map: &mut NodeScoreMap,
) -> ScoreNode {
    let pin = graph.in_pin(InPinId {
        node: node_id,
        input: input_idx,
    });

    if pin.remotes.is_empty() {
        return ScoreNode::Fixed(0.0);
    }

    let source_id = pin.remotes[0].node;

    if let Some(existing) = node_map.get(&source_id) {
        return existing.clone();
    }

    let source_node = &graph[source_id];

    let score = match source_node {
        EditorNode::Consider { consideration } => {
            ScoreNode::Consider(ConsiderationId::new(consideration))
        }
        EditorNode::Fixed { value } => ScoreNode::Fixed(*value),
        EditorNode::Not => {
            let child = trace_score_tree(graph, source_id, 0, node_map);
            ScoreNode::Not(Box::new(child))
        }
        EditorNode::Weighted { weight } => {
            let child = trace_score_tree(graph, source_id, 0, node_map);
            ScoreNode::Weighted(*weight, Box::new(child))
        }
        EditorNode::Curve { curve, .. } => {
            let child = trace_score_tree(graph, source_id, 0, node_map);
            ScoreNode::Curve(*curve, Box::new(child))
        }
        EditorNode::Clamp => {
            let child = trace_score_tree(graph, source_id, 0, node_map);
            let min = trace_score_tree_or(graph, source_id, 1, node_map, 0.0);
            let max = trace_score_tree_or(graph, source_id, 2, node_map, 1.0);
            ScoreNode::Clamp(Box::new(min), Box::new(max), Box::new(child))
        }
        EditorNode::Normalize => {
            let child = trace_score_tree(graph, source_id, 0, node_map);
            let min = trace_score_tree_or(graph, source_id, 1, node_map, 0.0);
            let max = trace_score_tree_or(graph, source_id, 2, node_map, 1.0);
            ScoreNode::Normalize(Box::new(min), Box::new(max), Box::new(child))
        }
        EditorNode::Mul { input_count }
        | EditorNode::Max { input_count }
        | EditorNode::Average { input_count }
        | EditorNode::Geom { input_count } => {
            let children: Vec<ScoreNode> = (0..*input_count)
                .map(|i| trace_score_tree(graph, source_id, i, node_map))
                .collect();
            match source_node {
                EditorNode::Mul { .. } => ScoreNode::Mul(children),
                EditorNode::Max { .. } => ScoreNode::Max(children),
                EditorNode::Average { .. } => ScoreNode::Average(children),
                EditorNode::Geom { .. } => ScoreNode::Geom(children),
                _ => unreachable!(),
            }
        }
        EditorNode::Composite { .. } | EditorNode::Behavior { .. } => ScoreNode::Fixed(0.0),
    };

    node_map.insert(source_id, score.clone());
    score
}

/// Like [`trace_score_tree`] but returns `Fixed(default)` when the pin is disconnected.
fn trace_score_tree_or(
    graph: &Snarl<EditorNode>,
    node_id: NodeId,
    input_idx: usize,
    node_map: &mut NodeScoreMap,
    default: f32,
) -> ScoreNode {
    let pin = graph.in_pin(InPinId {
        node: node_id,
        input: input_idx,
    });
    if pin.remotes.is_empty() {
        return ScoreNode::Fixed(default);
    }
    trace_score_tree(graph, node_id, input_idx, node_map)
}

// --- Profile → Graph conversion ---

/// Convert a `UtilityAIProfile` into an editor graph.
///
/// Behaviors with the same action name are merged into a single
/// `Behavior` node with multiple input pins, preserving the original
/// profile order within each group.
pub fn profile_to_graph(profile: &UtilityAIProfile) -> Snarl<EditorNode> {
    let mut graph = Snarl::new();

    for composite in &profile.composites {
        let root_id = graph.insert_node(
            egui::Pos2::ZERO,
            EditorNode::Composite {
                name: composite.id.as_str().to_owned(),
            },
        );

        let child_id = insert_score_node(&mut graph, &composite.score);
        connect(&mut graph, child_id, root_id, 0);
    }

    // Group behaviors by action name, preserving first-appearance order.
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&BehaviorDef>> = HashMap::new();

    for behavior in &profile.behaviors {
        let name = behavior.action.as_str().to_owned();
        if !groups.contains_key(&name) {
            group_order.push(name.clone());
        }
        groups.entry(name).or_default().push(behavior);
    }

    for name in &group_order {
        let defs = &groups[name];
        let root_id = graph.insert_node(
            egui::Pos2::ZERO,
            EditorNode::Behavior {
                behavior: name.clone(),
                input_count: defs.len(),
            },
        );

        for (i, def) in defs.iter().enumerate() {
            let child_id = insert_score_node(&mut graph, &def.score);
            connect(&mut graph, child_id, root_id, i);
        }
    }

    graph
}

fn insert_score_node(graph: &mut Snarl<EditorNode>, node: &ScoreNode) -> NodeId {
    match node {
        ScoreNode::Consider(consideration_id) => graph.insert_node(
            egui::Pos2::ZERO,
            EditorNode::Consider {
                consideration: consideration_id.as_str().to_owned(),
            },
        ),
        ScoreNode::Fixed(value) => {
            graph.insert_node(egui::Pos2::ZERO, EditorNode::Fixed { value: *value })
        }
        ScoreNode::Not(child) => {
            let id = graph.insert_node(egui::Pos2::ZERO, EditorNode::Not);
            let child_id = insert_score_node(graph, child);
            connect(graph, child_id, id, 0);
            id
        }
        ScoreNode::Weighted(weight, child) => {
            let id = graph.insert_node(egui::Pos2::ZERO, EditorNode::Weighted { weight: *weight });
            let child_id = insert_score_node(graph, child);
            connect(graph, child_id, id, 0);
            id
        }
        ScoreNode::Curve(curve, child) => {
            let id = graph.insert_node(
                egui::Pos2::ZERO,
                EditorNode::Curve {
                    curve: *curve,
                    collapsed: false,
                },
            );
            let child_id = insert_score_node(graph, child);
            connect(graph, child_id, id, 0);
            id
        }
        ScoreNode::Clamp(min, max, child) | ScoreNode::Normalize(min, max, child) => {
            let editor_node = match node {
                ScoreNode::Clamp(..) => EditorNode::Clamp,
                _ => EditorNode::Normalize,
            };
            let id = graph.insert_node(egui::Pos2::ZERO, editor_node);
            let child_id = insert_score_node(graph, child);
            connect(graph, child_id, id, 0);
            if !is_fixed(min, 0.0) {
                let min_id = insert_score_node(graph, min);
                connect(graph, min_id, id, 1);
            }
            if !is_fixed(max, 1.0) {
                let max_id = insert_score_node(graph, max);
                connect(graph, max_id, id, 2);
            }
            id
        }
        ScoreNode::Mul(children)
        | ScoreNode::Max(children)
        | ScoreNode::Average(children)
        | ScoreNode::Geom(children) => {
            let editor_node = match node {
                ScoreNode::Mul(_) => EditorNode::Mul {
                    input_count: children.len(),
                },
                ScoreNode::Max(_) => EditorNode::Max {
                    input_count: children.len(),
                },
                ScoreNode::Average(_) => EditorNode::Average {
                    input_count: children.len(),
                },
                ScoreNode::Geom(_) => EditorNode::Geom {
                    input_count: children.len(),
                },
                _ => unreachable!(),
            };
            let id = graph.insert_node(egui::Pos2::ZERO, editor_node);
            for (i, child) in children.iter().enumerate() {
                let child_id = insert_score_node(graph, child);
                connect(graph, child_id, id, i);
            }
            id
        }
    }
}

// --- Layout using actual rendered sizes ---

/// Universal margin between nodes in all directions.
const MARGIN: f32 = 25.0;

/// Lerp factor per frame for animated layout (0.0 = no movement, 1.0 = snap).
const LAYOUT_LERP: f32 = 0.25;

/// Repositions all nodes using actual rendered sizes from egui.
///
/// Consider and Fixed nodes are placed in the leftmost column, Behavior
/// in the rightmost, and intermediate nodes in columns between them.
/// Column widths and row heights are derived from the real node sizes.
pub fn apply_layout(graph: &mut Snarl<EditorNode>, node_sizes: &HashMap<NodeId, egui::Vec2>) {
    // Phase 1: collect graph structure (read-only)
    let mut behavior_roots = Vec::new();
    let mut children_of: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut is_source: HashMap<NodeId, bool> = HashMap::new();

    for (node_id, node) in graph.node_ids() {
        if matches!(
            node,
            EditorNode::Composite { .. } | EditorNode::Behavior { .. }
        ) {
            behavior_roots.push(node_id);
        }
        is_source.insert(
            node_id,
            matches!(node, EditorNode::Consider { .. } | EditorNode::Fixed { .. }),
        );

        let mut node_children = Vec::new();
        for i in 0..node.input_count() {
            let pin = graph.in_pin(InPinId {
                node: node_id,
                input: i,
            });
            for &remote in &pin.remotes {
                node_children.push(remote.node);
            }
        }
        children_of.insert(node_id, node_children);
    }

    // Compute depth from root for each reachable node
    let mut node_depths: HashMap<NodeId, usize> = HashMap::new();
    let mut max_depth: usize = 0;

    for &root_id in &behavior_roots {
        compute_depths(&children_of, root_id, 0, &mut node_depths, &mut max_depth);
    }

    // Assign columns: sources→0, roots→max_depth+1, others→max_depth+1-depth
    let num_cols = max_depth + 2;
    let mut node_columns: HashMap<NodeId, usize> = HashMap::new();

    for (&node_id, &depth) in &node_depths {
        let col = if is_source.get(&node_id).copied().unwrap_or(false) {
            0
        } else {
            (max_depth + 1).saturating_sub(depth)
        };
        node_columns.insert(node_id, col);
    }

    // Compute max width per column from actual sizes
    let mut col_widths = vec![0.0_f32; num_cols];
    for (&node_id, &col) in &node_columns {
        let w = node_sizes.get(&node_id).map(|s| s.x).unwrap_or(100.0);
        col_widths[col] = col_widths[col].max(w);
    }

    // Accumulate column X positions
    let mut col_x = vec![MARGIN; num_cols];
    for i in 1..num_cols {
        col_x[i] = col_x[i - 1] + col_widths[i - 1] + MARGIN;
    }

    // Phase 2: position nodes (mutable)
    let mut next_y = MARGIN;
    for &root_id in &behavior_roots {
        position_subtree(
            graph,
            root_id,
            &children_of,
            &node_columns,
            &col_x,
            node_sizes,
            &mut next_y,
        );
        next_y += MARGIN;
    }
}

fn compute_depths(
    children_of: &HashMap<NodeId, Vec<NodeId>>,
    node_id: NodeId,
    depth: usize,
    depths: &mut HashMap<NodeId, usize>,
    max_depth: &mut usize,
) {
    depths.insert(node_id, depth);
    *max_depth = (*max_depth).max(depth);

    if let Some(children) = children_of.get(&node_id) {
        for &child_id in children {
            compute_depths(children_of, child_id, depth + 1, depths, max_depth);
        }
    }
}

/// Positions a subtree using leaf-first ordering. Returns the vertical center.
fn position_subtree(
    graph: &mut Snarl<EditorNode>,
    node_id: NodeId,
    children_of: &HashMap<NodeId, Vec<NodeId>>,
    node_columns: &HashMap<NodeId, usize>,
    col_x: &[f32],
    node_sizes: &HashMap<NodeId, egui::Vec2>,
    next_y: &mut f32,
) -> f32 {
    let children = children_of.get(&node_id).cloned().unwrap_or_default();
    let col = node_columns.get(&node_id).copied().unwrap_or(0);
    let x = col_x.get(col).copied().unwrap_or(MARGIN);
    let h = node_sizes.get(&node_id).map(|s| s.y).unwrap_or(60.0);

    if children.is_empty() {
        let y = *next_y;
        *next_y += h + MARGIN;
        let target = egui::pos2(x, y);
        if let Some(info) = graph.get_node_info_mut(node_id) {
            info.pos = info.pos.lerp(target, LAYOUT_LERP);
        }
        return y + h / 2.0;
    }

    let child_centers: Vec<f32> = children
        .iter()
        .map(|&child_id| {
            position_subtree(
                graph,
                child_id,
                children_of,
                node_columns,
                col_x,
                node_sizes,
                next_y,
            )
        })
        .collect();

    let center_y = child_centers.iter().sum::<f32>() / child_centers.len() as f32;
    let target = egui::pos2(x, center_y - h / 2.0);
    if let Some(info) = graph.get_node_info_mut(node_id) {
        info.pos = info.pos.lerp(target, LAYOUT_LERP);
    }

    center_y
}

fn is_fixed(node: &ScoreNode, value: f32) -> bool {
    matches!(node, ScoreNode::Fixed(v) if (*v - value).abs() < f32::EPSILON)
}

fn connect(graph: &mut Snarl<EditorNode>, from: NodeId, to: NodeId, input: usize) {
    graph.connect(
        OutPinId {
            node: from,
            output: 0,
        },
        InPinId { node: to, input },
    );
}
