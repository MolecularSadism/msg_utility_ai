//! Turning a profile plus consideration values into a winning action.
//!
//! Both entry points resolve the profile's composites, score its behaviors, and
//! apply its [`PickStrategy`]. [`pick`] reports every behavior's score for
//! debug views; [`pick_action`] returns only the winner and allocates nothing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::{ConsiderationId, UtilityAIProfile};
use crate::scoring::{EvaluationResult, resolve_composites};

/// Strategy for selecting a winning behavior from scored options.
///
/// ```
/// use msg_utility_ai::picker::PickStrategy;
///
/// let strategy = PickStrategy::FirstToScore;
/// assert_ne!(strategy, PickStrategy::HighestScore);
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PickStrategy {
    /// First behavior above threshold wins (priority-ordered).
    #[default]
    FirstToScore,
    /// Highest-scoring behavior above threshold wins.
    HighestScore,
}

/// Evaluate all behaviors in a profile and select the winning action.
///
/// Scores every behavior, then applies the configured strategy to pick a winner.
/// Returns `None` for the action when nothing scores above threshold.
///
/// To guarantee a fallback, include a behavior with a `Fixed` score
/// at or above the threshold (e.g. `Fixed(0.5)` with threshold `0.5`).
///
/// ```
/// use msg_utility_ai::*;
/// use msg_utility_ai::picker::{PickStrategy, pick};
/// use std::collections::HashMap;
///
/// let profile = UtilityAIProfile {
///     threshold: 0.5,
///     strategy: PickStrategy::HighestScore,
///     composites: vec![],
///     behaviors: vec![
///         BehaviorDef {
///             action: ActionId::new("Chase"),
///             score: ScoreNode::Fixed(0.6),
///         },
///         BehaviorDef {
///             action: ActionId::new("Attack"),
///             score: ScoreNode::Fixed(0.9),
///         },
///     ],
/// };
///
/// let values = HashMap::new();
/// let result = pick(&profile, &values);
/// assert_eq!(result.action, Some(ActionId::new("Attack")));
/// ```
pub fn pick(
    profile: &UtilityAIProfile,
    consideration_values: &HashMap<ConsiderationId, f32>,
) -> EvaluationResult {
    // Resolve composite considerations before scoring behaviors
    let mut values;
    let effective_values = if profile.composites.is_empty() {
        consideration_values
    } else {
        values = consideration_values.clone();
        // Cycle errors are silently ignored — composites involved in cycles
        // remain unresolved (missing considerations evaluate to 0.0).
        let _ = resolve_composites(&profile.composites, &mut values);
        &values
    };

    let mut scores = Vec::with_capacity(profile.behaviors.len());

    for behavior in &profile.behaviors {
        let score = behavior.score.evaluate(effective_values);
        scores.push(score);
    }

    let winner = match profile.strategy {
        PickStrategy::FirstToScore => scores.iter().position(|&score| score >= profile.threshold),
        PickStrategy::HighestScore => {
            let mut best: Option<(usize, f32)> = None;
            for (i, &score) in scores.iter().enumerate() {
                if score >= profile.threshold && (best.is_none() || score > best.unwrap().1) {
                    best = Some((i, score));
                }
            }
            best.map(|(i, _)| i)
        }
    };

    let action = winner.map(|i| profile.behaviors[i].action);

    EvaluationResult {
        scores,
        winner,
        action,
    }
}

/// Evaluate a profile and return only the winning action.
///
/// Allocation-free variant of [`pick`] for hot paths that don't need the full
/// per-behavior score list (the runtime evaluation loop, as opposed to
/// editors/debug views). [`PickStrategy::FirstToScore`] stops scoring at the
/// first behavior that clears the threshold.
///
/// ```
/// use msg_utility_ai::*;
/// use msg_utility_ai::picker::{PickStrategy, pick_action};
/// use std::collections::HashMap;
///
/// let profile = UtilityAIProfile {
///     threshold: 0.5,
///     strategy: PickStrategy::HighestScore,
///     composites: vec![],
///     behaviors: vec![
///         BehaviorDef {
///             action: ActionId::new("Chase"),
///             score: ScoreNode::Fixed(0.6),
///         },
///         BehaviorDef {
///             action: ActionId::new("Attack"),
///             score: ScoreNode::Fixed(0.9),
///         },
///     ],
/// };
///
/// let values = HashMap::new();
/// assert_eq!(pick_action(&profile, &values), Some(ActionId::new("Attack")));
/// ```
// TODO: Duplicates the composite-resolution preamble in `pick` verbatim, including its
// silent handling of cycle errors. Factor the shared part out.
pub fn pick_action(
    profile: &UtilityAIProfile,
    consideration_values: &HashMap<ConsiderationId, f32>,
) -> Option<crate::config::ActionId> {
    let mut values;
    let effective_values = if profile.composites.is_empty() {
        consideration_values
    } else {
        values = consideration_values.clone();
        // Cycle errors are silently ignored — composites involved in cycles
        // remain unresolved (missing considerations evaluate to 0.0).
        let _ = resolve_composites(&profile.composites, &mut values);
        &values
    };

    match profile.strategy {
        PickStrategy::FirstToScore => profile
            .behaviors
            .iter()
            .find(|behavior| behavior.score.evaluate(effective_values) >= profile.threshold)
            .map(|behavior| behavior.action),
        PickStrategy::HighestScore => {
            let mut best: Option<(crate::config::ActionId, f32)> = None;
            for behavior in &profile.behaviors {
                let score = behavior.score.evaluate(effective_values);
                if score >= profile.threshold && best.is_none_or(|(_, b)| score > b) {
                    best = Some((behavior.action, score));
                }
            }
            best.map(|(action, _)| action)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActionId, BehaviorDef};
    use crate::scoring::ScoreNode;

    fn s(name: &str) -> ConsiderationId {
        ConsiderationId::new(name)
    }

    fn a(name: &str) -> ActionId {
        ActionId::new(name)
    }

    fn consideration_values(entries: &[(ConsiderationId, f32)]) -> HashMap<ConsiderationId, f32> {
        entries.iter().copied().collect()
    }

    #[test]
    fn test_first_to_score_picks_first_above_threshold() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::FirstToScore,
            composites: vec![],
            behaviors: vec![
                BehaviorDef {
                    action: a("Chase"),
                    score: ScoreNode::Consider(s("has_target")),
                },
                BehaviorDef {
                    action: a("Attack"),
                    score: ScoreNode::Consider(s("in_range")),
                },
            ],
        };

        let values = consideration_values(&[(s("has_target"), 0.8), (s("in_range"), 0.9)]);
        let result = pick(&profile, &values);
        assert_eq!(result.action, Some(a("Chase")));
        assert_eq!(result.winner, Some(0));
    }

    #[test]
    fn test_highest_score_picks_best() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::HighestScore,
            composites: vec![],
            behaviors: vec![
                BehaviorDef {
                    action: a("Chase"),
                    score: ScoreNode::Fixed(0.6),
                },
                BehaviorDef {
                    action: a("Attack"),
                    score: ScoreNode::Fixed(0.9),
                },
            ],
        };

        let values = consideration_values(&[]);
        let result = pick(&profile, &values);
        assert_eq!(result.action, Some(a("Attack")));
        assert_eq!(result.winner, Some(1));
    }

    #[test]
    fn test_no_winner_returns_none() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::FirstToScore,
            composites: vec![],
            behaviors: vec![BehaviorDef {
                action: a("Attack"),
                score: ScoreNode::Fixed(0.2),
            }],
        };

        let values = consideration_values(&[]);
        let result = pick(&profile, &values);
        assert_eq!(result.action, None);
        assert_eq!(result.winner, None);
    }

    #[test]
    fn test_highest_score_ignores_below_threshold() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::HighestScore,
            composites: vec![],
            behaviors: vec![
                BehaviorDef {
                    action: a("Chase"),
                    score: ScoreNode::Fixed(0.4),
                },
                BehaviorDef {
                    action: a("Attack"),
                    score: ScoreNode::Fixed(0.3),
                },
            ],
        };

        let values = consideration_values(&[]);
        let result = pick(&profile, &values);
        assert_eq!(result.action, None);
        assert_eq!(result.winner, None);
    }

    #[test]
    fn test_momentum_as_composed_consideration() {
        // Momentum is just another consideration wired into the score tree.
        // The game sets "current_action:Chase" to 1.0 when in Chase state.
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::FirstToScore,
            composites: vec![],
            behaviors: vec![BehaviorDef {
                action: a("Chase"),
                score: ScoreNode::Average(vec![
                    ScoreNode::Weighted(0.8, Box::new(ScoreNode::Consider(s("has_target")))),
                    ScoreNode::Weighted(
                        0.2,
                        Box::new(ScoreNode::Consider(s("current_action:Chase"))),
                    ),
                ]),
            }],
        };

        // Without momentum, avg(0.8 * 0.9, 0.2 * 0.0) = 0.36 -- below threshold
        let values = consideration_values(&[(s("has_target"), 0.9)]);
        let result = pick(&profile, &values);
        assert_eq!(result.action, None);

        // With momentum, avg(0.8 * 0.9, 0.2 * 1.0) = 0.46 still below, but
        // with full target: avg(0.8 * 1.0, 0.2 * 1.0) = 0.5 -- at threshold
        let values =
            consideration_values(&[(s("has_target"), 1.0), (s("current_action:Chase"), 1.0)]);
        let result = pick(&profile, &values);
        assert_eq!(result.action, Some(a("Chase")));
    }
}
