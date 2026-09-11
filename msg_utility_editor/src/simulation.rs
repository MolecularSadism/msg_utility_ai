use std::collections::HashMap;

use egui_snarl::NodeId;
use msg_utility_ai::picker::PickStrategy;
use msg_utility_ai::scoring::resolve_composites;
use msg_utility_ai::{ConsiderationId, evaluate_profile};

use crate::document::{BehaviorGroupScore, SimResult};
use crate::graph::EditorNode;
use crate::graph::graph_to_profile;

pub fn run_simulation(
    graph: &egui_snarl::Snarl<EditorNode>,
    sensor_values: &HashMap<String, f32>,
    threshold: f32,
    strategy: PickStrategy,
) -> (SimResult, HashMap<NodeId, f32>, HashMap<NodeId, Vec<f32>>) {
    let (profile, node_map, origins) = graph_to_profile(graph, threshold, strategy);

    let mut consideration_map: HashMap<ConsiderationId, f32> = sensor_values
        .iter()
        .map(|(name, &val)| (ConsiderationId::new(name), val))
        .collect();

    let result = evaluate_profile(&profile, &consideration_map);

    // Build raw (name, score) pairs in profile order.
    let raw_scores: Vec<(String, f32)> = profile
        .behaviors
        .iter()
        .zip(result.scores.iter())
        .map(|(b, &s)| (b.action.as_str().to_owned(), s))
        .collect();

    let winner = result.winner.map(|i| raw_scores[i].0.clone());

    // Resolve composites so per-node scores reflect their computed values.
    let _ = resolve_composites(&profile.composites, &mut consideration_map);

    // Build per-node behavior thought scores for the viewer.
    let mut behavior_scores: HashMap<NodeId, Vec<f32>> = HashMap::new();
    for (origin, &score) in origins.iter().zip(result.scores.iter()) {
        behavior_scores
            .entry(origin.node_id)
            .or_default()
            .push(score);
    }

    // Group scores by behavior name for the properties panel.
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<f32>> = HashMap::new();
    for (name, score) in &raw_scores {
        if !groups.contains_key(name) {
            group_order.push(name.clone());
        }
        groups.entry(name.clone()).or_default().push(*score);
    }

    let behavior_groups: Vec<BehaviorGroupScore> = group_order
        .iter()
        .map(|name| {
            let thoughts = groups[name].clone();
            let top_score = thoughts.iter().cloned().fold(0.0_f32, f32::max);
            BehaviorGroupScore {
                name: name.clone(),
                top_score,
                thoughts,
                is_winner: winner.as_ref() == Some(name),
            }
        })
        .collect();

    // Per-node scores for intermediate node visualization.
    let node_scores: HashMap<NodeId, f32> = node_map
        .iter()
        .map(|(&node_id, score_node)| (node_id, score_node.evaluate(&consideration_map)))
        .collect();

    (
        SimResult {
            groups: behavior_groups,
            winner,
        },
        node_scores,
        behavior_scores,
    )
}
