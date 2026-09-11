//! The serialized side of the system: profiles, behaviors, composites, and the
//! interned IDs that tie them to the game.

use bevy::prelude::*;
use bevy_interned_id::interned_id;
use serde::{Deserialize, Serialize};

use crate::picker::PickStrategy;
use crate::scoring::ScoreNode;

interned_id!(
    /// Opaque consideration identifier. The game defines what considerations exist.
    ///
    /// ```
    /// use msg_utility_ai::ConsiderationId;
    ///
    /// let id = ConsiderationId::new("has_target");
    /// assert_eq!(id, ConsiderationId::new("has_target"));
    /// ```
    pub ConsiderationId
);

interned_id!(
    /// Opaque action identifier. The game maps these to FSM states.
    ///
    /// ```
    /// use msg_utility_ai::ActionId;
    ///
    /// let id = ActionId::new("Chase");
    /// assert_eq!(id, ActionId::new("Chase"));
    /// ```
    pub ActionId
);

/// Top-level AI profile, loaded from `.ai.ron` files.
///
/// There is no built-in fallback action: to guarantee one, end the behavior list with a
/// [`ScoreNode::Fixed`] score at or above `threshold`, or append a shared one via
/// [`DefaultOccupation`].
///
/// ```
/// use msg_utility_ai::*;
///
/// let profile = UtilityAIProfile {
///     threshold: 0.5,
///     strategy: msg_utility_ai::picker::PickStrategy::default(),
///     composites: vec![],
///     behaviors: vec![
///         BehaviorDef {
///             action: ActionId::new("Idle"),
///             score: ScoreNode::Fixed(0.5),
///         },
///         BehaviorDef {
///             action: ActionId::new("AttackPrimary"),
///             score: ScoreNode::Mul(vec![
///                 ScoreNode::Consider(ConsiderationId::new("has_target")),
///                 ScoreNode::Consider(ConsiderationId::new("in_attack_range")),
///             ]),
///         },
///     ],
/// };
///
/// assert_eq!(profile.behaviors.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Asset, TypePath)]
pub struct UtilityAIProfile {
    /// Minimum score a behavior must reach to be eligible to win.
    pub threshold: f32,
    /// How a winner is chosen among the behaviors that clear `threshold`.
    #[serde(default)]
    pub strategy: PickStrategy,
    /// Named subexpressions resolved into the consideration map before scoring.
    #[serde(default)]
    pub composites: Vec<CompositeConsiderationDef>,
    /// The candidate actions. Order matters under [`PickStrategy::FirstToScore`].
    pub behaviors: Vec<BehaviorDef>,
}

/// A game's shared fallback behavior, appended to profiles via
/// [`UtilityAIProfile::register_default_occupation`].
///
/// ```
/// use msg_utility_ai::*;
///
/// struct MyGameDefaults;
///
/// impl DefaultOccupation for MyGameDefaults {
///     fn default_occupation() -> BehaviorDef {
///         BehaviorDef {
///             action: ActionId::new("Idle"),
///             score: ScoreNode::Fixed(0.5),
///         }
///     }
/// }
///
/// let mut profile = UtilityAIProfile {
///     threshold: 0.5,
///     strategy: msg_utility_ai::picker::PickStrategy::default(),
///     composites: vec![],
///     behaviors: vec![],
/// };
/// profile.register_default_occupation::<MyGameDefaults>();
/// assert_eq!(profile.behaviors.len(), 1);
/// assert_eq!(profile.behaviors[0].action, ActionId::new("Idle"));
/// ```
pub trait DefaultOccupation {
    /// Returns the default fallback behavior definition.
    fn default_occupation() -> BehaviorDef;
}

impl UtilityAIProfile {
    /// Appends the default occupation behavior from the given [`DefaultOccupation`] implementor.
    ///
    /// The behavior is added at the end of the list so it only wins when no
    /// other behavior scores above the threshold (first-to-score strategy).
    pub fn register_default_occupation<T: DefaultOccupation>(&mut self) {
        self.behaviors.push(T::default_occupation());
    }

    /// Collect all consideration IDs referenced by all behaviors and composites in this profile.
    pub fn referenced_considerations(&self) -> Vec<ConsiderationId> {
        let mut ids = Vec::new();
        for behavior in &self.behaviors {
            behavior.score.collect_considerations(&mut ids);
        }
        for composite in &self.composites {
            composite.score.collect_considerations(&mut ids);
        }
        ids.sort_by_key(|s| s.as_str().to_owned());
        ids.dedup();
        ids
    }
}

/// One candidate action and the expression that scores it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorDef {
    /// Reported as the winner when this behavior is picked.
    pub action: ActionId,
    /// Evaluated against the entity's consideration values.
    pub score: ScoreNode,
}

/// A composite consideration: a named value computed from a [`ScoreNode`] expression tree.
///
/// Composite considerations are evaluated before behavior scoring, and their results
/// are injected into the consideration values map. This allows `Consider("composite_name")`
/// in behavior trees to reference precomputed values, reducing duplication.
///
/// ```
/// use msg_utility_ai::*;
///
/// let composite = CompositeConsiderationDef {
///     id: ConsiderationId::new("target_detected"),
///     score: ScoreNode::Max(vec![
///         ScoreNode::Consider(ConsiderationId::new("can_see_player")),
///         ScoreNode::Consider(ConsiderationId::new("in_hearing_range")),
///     ]),
/// };
///
/// assert_eq!(composite.id, ConsiderationId::new("target_detected"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeConsiderationDef {
    /// The consideration ID this composite's result is stored under.
    pub id: ConsiderationId,
    /// May reference raw considerations and other composites.
    pub score: ScoreNode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consideration_id_equality() {
        assert_eq!(
            ConsiderationId::new("has_target"),
            ConsiderationId::new("has_target")
        );
        assert_ne!(
            ConsiderationId::new("has_target"),
            ConsiderationId::new("in_range")
        );
    }

    #[test]
    fn test_action_id_equality() {
        assert_eq!(ActionId::new("Chase"), ActionId::new("Chase"));
        assert_ne!(ActionId::new("Chase"), ActionId::new("Idle"));
    }

    #[test]
    fn test_ron_roundtrip_profile() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![],
            behaviors: vec![
                BehaviorDef {
                    action: ActionId::new("AttackPrimary"),
                    score: ScoreNode::Mul(vec![
                        ScoreNode::Consider(ConsiderationId::new("has_target")),
                        ScoreNode::Not(Box::new(ScoreNode::Consider(ConsiderationId::new(
                            "in_attack_range",
                        )))),
                    ]),
                },
                BehaviorDef {
                    action: ActionId::new("Chase"),
                    score: ScoreNode::Mul(vec![
                        ScoreNode::Consider(ConsiderationId::new("has_target")),
                        ScoreNode::Max(vec![
                            ScoreNode::Consider(ConsiderationId::new("can_see_player")),
                            ScoreNode::Consider(ConsiderationId::new("in_attack_range")),
                        ]),
                    ]),
                },
                BehaviorDef {
                    action: ActionId::new("Idle"),
                    score: ScoreNode::Fixed(0.5),
                },
            ],
        };

        let ron_str =
            ron::ser::to_string_pretty(&profile, ron::ser::PrettyConfig::default()).unwrap();
        let deserialized: UtilityAIProfile = ron::from_str(&ron_str).unwrap();

        assert_eq!(deserialized.behaviors.len(), 3);
        assert_eq!(deserialized.threshold, 0.5);
    }

    #[test]
    fn test_register_default_occupation() {
        struct GameDefaults;

        impl DefaultOccupation for GameDefaults {
            fn default_occupation() -> BehaviorDef {
                BehaviorDef {
                    action: ActionId::new("Idle"),
                    score: ScoreNode::Fixed(0.5),
                }
            }
        }

        let mut profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![],
            behaviors: vec![],
        };
        profile.register_default_occupation::<GameDefaults>();

        assert_eq!(profile.behaviors.len(), 1);
        assert_eq!(profile.behaviors[0].action, ActionId::new("Idle"));
    }

    #[test]
    fn test_ron_deserialize_behavior() {
        let ron_str = r#"(
            action: "Chase",
            score: Consider("has_target"),
        )"#;
        let behavior: BehaviorDef = ron::from_str(ron_str).unwrap();
        assert_eq!(behavior.action, ActionId::new("Chase"));
    }
}
