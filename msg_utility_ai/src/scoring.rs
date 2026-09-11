//! The scoring expression tree and the response curves its leaves feed into.
//!
//! [`ScoreNode::evaluate`] walks a tree against a consideration map and yields one number per
//! behavior. Composite considerations are resolved first by [`resolve_composites`], which
//! topologically sorts them so a composite may build on another and reports a
//! [`CompositeError::Cycle`] when they refer back to each other.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::config::{ActionId, CompositeConsiderationDef, ConsiderationId, UtilityAIProfile};

/// Response curve type for utility AI scoring.
///
/// Each variant uses the four tuning parameters (m, k, b, c) differently:
/// - `Linear`: `y = m * (x - c) + b`
/// - `Exponential`: `y = m * (x - c)^k + b`
/// - `Logistic`: `y = m / (1 + e^(-k * (x - c))) + b`
///
/// ```
/// use msg_utility_ai::scoring::CurveType;
///
/// let variants = [CurveType::Linear, CurveType::Exponential, CurveType::Logistic];
/// assert_eq!(variants.len(), 3);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CurveType {
    Linear,
    Exponential,
    Logistic,
}

impl CurveType {
    /// Every variant, for editors and debug views that let a user cycle curves.
    pub const ALL: &[CurveType] = &[
        CurveType::Linear,
        CurveType::Exponential,
        CurveType::Logistic,
    ];
}

/// Parameterized response curve with four tuning parameters.
///
/// - `m`: slope / multiplier
/// - `k`: exponent (Exponential) or steepness (Logistic)
/// - `b`: vertical shift
/// - `c`: horizontal shift
///
/// ```
/// use msg_utility_ai::scoring::ResponseCurve;
///
/// let curve = ResponseCurve::default();
/// let y = curve.sample(0.5);
/// assert!((y - 0.5).abs() < f32::EPSILON);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ResponseCurve {
    pub curve_type: CurveType,
    pub m: f32,
    pub k: f32,
    pub b: f32,
    pub c: f32,
}

impl Default for ResponseCurve {
    fn default() -> Self {
        Self {
            curve_type: CurveType::Linear,
            m: 1.0,
            k: 1.0,
            b: 0.0,
            c: 0.0,
        }
    }
}

impl ResponseCurve {
    /// Sample the curve at `x`, clamping output to [0, 1].
    pub fn sample(&self, x: f32) -> f32 {
        let y = match self.curve_type {
            CurveType::Linear => self.m * (x - self.c) + self.b,
            CurveType::Exponential => {
                let base = x - self.c;
                let powered = if base < 0.0 && (self.k.fract()).abs() > f32::EPSILON {
                    // Negative base with non-integer exponent: use sign-preserving power
                    -((-base).powf(self.k))
                } else {
                    base.powf(self.k)
                };
                self.m * powered + self.b
            }
            CurveType::Logistic => {
                let exponent = -self.k * (x - self.c);
                self.m / (1.0 + exponent.exp()) + self.b
            }
        };
        y.clamp(0.0, 1.0)
    }
}

/// Composable scoring expression tree.
///
/// A single recursive enum that reads naturally in RON files.
/// Considerations and actions are referenced by opaque string IDs.
///
/// ```
/// use msg_utility_ai::*;
///
/// let tree = ScoreNode::Mul(vec![
///     ScoreNode::Consider(ConsiderationId::new("has_target")),
///     ScoreNode::Not(Box::new(ScoreNode::Consider(ConsiderationId::new("in_attack_range")))),
/// ]);
///
/// let mut values = std::collections::HashMap::new();
/// values.insert(ConsiderationId::new("has_target"), 1.0);
/// values.insert(ConsiderationId::new("in_attack_range"), 0.3);
///
/// let score = tree.evaluate(&values);
/// assert!((score - 0.7).abs() < f32::EPSILON);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScoreNode {
    /// Read a consideration value by ID.
    Consider(ConsiderationId),
    /// Multiply children. Any zero kills the entire score.
    Mul(Vec<ScoreNode>),
    /// Take the max of children.
    Max(Vec<ScoreNode>),
    /// Invert: `1.0 - child_score`.
    Not(Box<ScoreNode>),
    /// Average children (mean of child scores).
    Average(Vec<ScoreNode>),
    /// Geometric mean of children.
    Geom(Vec<ScoreNode>),
    /// Fixed constant value.
    Fixed(f32),
    /// Apply a weight multiplier to a child node.
    Weighted(f32, Box<ScoreNode>),
    /// Apply a response curve to a child node's output.
    Curve(ResponseCurve, Box<ScoreNode>),
    /// Clamp a child node's output to `[min, max]`.
    /// Fields: `(min, max, child)` — all evaluated as score nodes.
    Clamp(Box<ScoreNode>, Box<ScoreNode>, Box<ScoreNode>),
    /// Map a child node's output from `[min, max]` to `[0.0, 1.0]`.
    ///
    /// Output is clamped to `[0.0, 1.0]`.
    /// Fields: `(min, max, child)` — all evaluated as score nodes.
    Normalize(Box<ScoreNode>, Box<ScoreNode>, Box<ScoreNode>),
}

impl ScoreNode {
    /// Evaluate this score tree against a map of consideration values.
    ///
    /// Consideration values should be in the 0.0-1.0 range (already eased/curved).
    /// Missing considerations evaluate to 0.0.
    pub fn evaluate(&self, consideration_values: &HashMap<ConsiderationId, f32>) -> f32 {
        match self {
            ScoreNode::Consider(consideration_id) => consideration_values
                .get(consideration_id)
                .copied()
                .unwrap_or(0.0),
            ScoreNode::Mul(children) => {
                let mut result = 1.0;
                for child in children {
                    let score = child.evaluate(consideration_values);
                    if score <= 0.0 {
                        return 0.0;
                    }
                    result *= score;
                }
                result
            }
            ScoreNode::Max(children) => {
                let mut max = 0.0_f32;
                for child in children {
                    let score = child.evaluate(consideration_values);
                    if score >= 1.0 {
                        return 1.0;
                    }
                    max = max.max(score);
                }
                max
            }
            ScoreNode::Not(child) => 1.0 - child.evaluate(consideration_values).clamp(0.0, 1.0),
            ScoreNode::Average(children) => {
                if children.is_empty() {
                    return 0.0;
                }
                let mut total = 0.0;
                for child in children {
                    total += child.evaluate(consideration_values);
                }
                total / children.len() as f32
            }
            ScoreNode::Geom(children) => {
                if children.is_empty() {
                    return 1.0;
                }
                let mut product = 1.0_f32;
                let n = children.len() as f32;
                for child in children {
                    product *= child.evaluate(consideration_values);
                }
                product.powf(1.0 / n).clamp(0.0, 1.0)
            }
            ScoreNode::Fixed(value) => *value,
            ScoreNode::Weighted(weight, child) => weight * child.evaluate(consideration_values),
            ScoreNode::Curve(curve, child) => {
                let raw = child.evaluate(consideration_values).clamp(0.0, 1.0);
                curve.sample(raw)
            }
            ScoreNode::Clamp(min, max, child) => {
                let min = min.evaluate(consideration_values);
                let max = max.evaluate(consideration_values);
                child.evaluate(consideration_values).clamp(min, max)
            }
            ScoreNode::Normalize(min, max, child) => {
                let min = min.evaluate(consideration_values);
                let max = max.evaluate(consideration_values);
                let raw = child.evaluate(consideration_values);
                if (max - min).abs() < f32::EPSILON {
                    if raw >= max { 1.0 } else { 0.0 }
                } else {
                    ((raw - min) / (max - min)).clamp(0.0, 1.0)
                }
            }
        }
    }

    /// Wrap this node in a [`Clamp`](ScoreNode::Clamp).
    ///
    /// ```
    /// use msg_utility_ai::*;
    ///
    /// let node = ScoreNode::Fixed(1.5).clamped(0.0, 1.0);
    /// let score = node.evaluate(&Default::default());
    /// assert!((score - 1.0).abs() < f32::EPSILON);
    /// ```
    pub fn clamped(self, min: f32, max: f32) -> Self {
        ScoreNode::Clamp(
            Box::new(ScoreNode::Fixed(min)),
            Box::new(ScoreNode::Fixed(max)),
            Box::new(self),
        )
    }

    /// Wrap this node in a [`Normalize`](ScoreNode::Normalize).
    ///
    /// Maps the child's output from `[min, max]` to `[0.0, 1.0]`.
    /// Output is clamped to `[0.0, 1.0]`.
    ///
    /// ```
    /// use msg_utility_ai::*;
    ///
    /// let node = ScoreNode::Fixed(75.0).normalized(50.0, 100.0);
    /// let score = node.evaluate(&Default::default());
    /// assert!((score - 0.5).abs() < f32::EPSILON);
    /// ```
    pub fn normalized(self, min: f32, max: f32) -> Self {
        ScoreNode::Normalize(
            Box::new(ScoreNode::Fixed(min)),
            Box::new(ScoreNode::Fixed(max)),
            Box::new(self),
        )
    }

    /// Every consideration ID this tree reads, sorted and deduplicated.
    ///
    /// ```
    /// use msg_utility_ai::*;
    ///
    /// let tree = ScoreNode::Mul(vec![
    ///     ScoreNode::Consider(ConsiderationId::new("has_target")),
    ///     ScoreNode::Max(vec![
    ///         ScoreNode::Consider(ConsiderationId::new("can_see_player")),
    ///         ScoreNode::Consider(ConsiderationId::new("in_attack_range")),
    ///     ]),
    /// ]);
    ///
    /// let ids = tree.referenced_considerations();
    /// assert_eq!(ids.len(), 3);
    /// ```
    pub fn referenced_considerations(&self) -> Vec<ConsiderationId> {
        let mut ids = Vec::new();
        self.collect_considerations(&mut ids);
        ids.sort_by_key(|s| s.as_str().to_owned());
        ids.dedup();
        ids
    }

    pub(crate) fn collect_considerations(&self, ids: &mut Vec<ConsiderationId>) {
        match self {
            ScoreNode::Consider(consideration_id) => ids.push(*consideration_id),
            ScoreNode::Mul(children)
            | ScoreNode::Max(children)
            | ScoreNode::Average(children)
            | ScoreNode::Geom(children) => {
                for child in children {
                    child.collect_considerations(ids);
                }
            }
            ScoreNode::Not(child) | ScoreNode::Weighted(_, child) | ScoreNode::Curve(_, child) => {
                child.collect_considerations(ids);
            }
            ScoreNode::Clamp(min, max, child) | ScoreNode::Normalize(min, max, child) => {
                min.collect_considerations(ids);
                max.collect_considerations(ids);
                child.collect_considerations(ids);
            }
            ScoreNode::Fixed(_) => {}
        }
    }
}

/// Error returned when composite consideration resolution fails.
///
/// ```
/// use msg_utility_ai::scoring::CompositeError;
/// use msg_utility_ai::ConsiderationId;
///
/// let err = CompositeError::Cycle(vec![ConsiderationId::new("a"), ConsiderationId::new("b")]);
/// assert!(format!("{err}").contains("Cycle"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositeError {
    /// A dependency cycle was detected among composite considerations.
    Cycle(Vec<ConsiderationId>),
}

impl fmt::Display for CompositeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompositeError::Cycle(ids) => {
                write!(f, "Cycle detected among composite considerations: [")?;
                for (i, id) in ids.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", id.as_str())?;
                }
                write!(f, "]")
            }
        }
    }
}

/// Evaluate composite considerations in dependency order, inserting results into
/// `consideration_values`.
///
/// Composite considerations can reference raw considerations and other composites. This function
/// performs a topological sort to ensure composites are evaluated in the correct order,
/// then evaluates each one and inserts the result into the consideration values map.
///
/// Returns `Err` if a dependency cycle is detected.
///
/// ```
/// use msg_utility_ai::*;
/// use msg_utility_ai::scoring::resolve_composites;
/// use std::collections::HashMap;
///
/// let composites = vec![
///     CompositeConsiderationDef {
///         id: ConsiderationId::new("target_detected"),
///         score: ScoreNode::Max(vec![
///             ScoreNode::Consider(ConsiderationId::new("can_see")),
///             ScoreNode::Consider(ConsiderationId::new("can_hear")),
///         ]),
///     },
/// ];
///
/// let mut values = HashMap::new();
/// values.insert(ConsiderationId::new("can_see"), 0.0);
/// values.insert(ConsiderationId::new("can_hear"), 0.8);
///
/// resolve_composites(&composites, &mut values).unwrap();
/// assert!((values[&ConsiderationId::new("target_detected")] - 0.8).abs() < f32::EPSILON);
/// ```
pub fn resolve_composites(
    composites: &[CompositeConsiderationDef],
    consideration_values: &mut HashMap<ConsiderationId, f32>,
) -> Result<(), CompositeError> {
    if composites.is_empty() {
        return Ok(());
    }

    let order = topological_sort(composites)?;

    for idx in order {
        let composite = &composites[idx];
        let value = composite.score.evaluate(consideration_values);
        consideration_values.insert(composite.id, value);
    }

    Ok(())
}

/// Topological sort of composite considerations by their dependencies.
///
/// Uses Kahn's algorithm. Only considers dependencies between composites
/// (references to raw considerations are ignored since they're already in the map).
fn topological_sort(
    composites: &[CompositeConsiderationDef],
) -> Result<Vec<usize>, CompositeError> {
    let composite_ids: HashSet<ConsiderationId> = composites.iter().map(|c| c.id).collect();
    let id_to_index: HashMap<ConsiderationId, usize> = composites
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id, i))
        .collect();

    // Build adjacency list and in-degree count
    let len = composites.len();
    let mut in_degree = vec![0u32; len];
    let mut dependents: Vec<Vec<usize>> = vec![vec![]; len];

    for (i, composite) in composites.iter().enumerate() {
        for dep in composite.score.referenced_considerations() {
            if composite_ids.contains(&dep) {
                if dep == composite.id {
                    return Err(CompositeError::Cycle(vec![composite.id]));
                }
                let dep_idx = id_to_index[&dep];
                dependents[dep_idx].push(i);
                in_degree[i] += 1;
            }
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(len);
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        for &dep in &dependents[idx] {
            in_degree[dep] -= 1;
            if in_degree[dep] == 0 {
                queue.push_back(dep);
            }
        }
    }

    if order.len() != len {
        // Collect IDs involved in cycle
        let cycle_ids: Vec<ConsiderationId> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, deg)| **deg > 0)
            .map(|(i, _)| composites[i].id)
            .collect();
        return Err(CompositeError::Cycle(cycle_ids));
    }

    Ok(order)
}

/// Result of evaluating all behaviors in a profile.
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    /// Scores per behavior, in profile order.
    pub scores: Vec<f32>,
    /// Index of the winning behavior, or `None` if no behavior scored above threshold.
    pub winner: Option<usize>,
    /// The winning action ID, or `None` if no behavior scored above threshold.
    pub action: Option<ActionId>,
}

/// Evaluate all behaviors in a profile against consideration values.
///
/// Delegates to [`crate::picker::pick`] using the profile's configured strategy.
///
/// ```
/// use msg_utility_ai::*;
/// use std::collections::HashMap;
///
/// let profile = UtilityAIProfile {
///     threshold: 0.5,
///     strategy: msg_utility_ai::picker::PickStrategy::default(),
///     composites: vec![],
///     behaviors: vec![
///         BehaviorDef {
///             action: ActionId::new("AttackPrimary"),
///             score: ScoreNode::Consider(ConsiderationId::new("in_attack_range")),
///         },
///         BehaviorDef {
///             action: ActionId::new("Idle"),
///             score: ScoreNode::Fixed(0.5),
///         },
///     ],
/// };
///
/// let mut values = HashMap::new();
/// values.insert(ConsiderationId::new("in_attack_range"), 0.8);
///
/// let result = evaluate_profile(&profile, &values);
/// assert_eq!(result.action, Some(ActionId::new("AttackPrimary")));
/// ```
pub fn evaluate_profile(
    profile: &UtilityAIProfile,
    consideration_values: &HashMap<ConsiderationId, f32>,
) -> EvaluationResult {
    crate::picker::pick(profile, consideration_values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BehaviorDef, CompositeConsiderationDef};
    use crate::picker::PickStrategy;

    fn consideration_values(entries: &[(ConsiderationId, f32)]) -> HashMap<ConsiderationId, f32> {
        entries.iter().copied().collect()
    }

    fn s(name: &str) -> ConsiderationId {
        ConsiderationId::new(name)
    }

    fn a(name: &str) -> ActionId {
        ActionId::new(name)
    }

    #[test]
    fn test_consider_node() {
        let node = ScoreNode::Consider(s("has_target"));
        let values = consideration_values(&[(s("has_target"), 0.8)]);
        assert!((node.evaluate(&values) - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_consider_missing_consideration() {
        let node = ScoreNode::Consider(s("has_target"));
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_and_node() {
        let node = ScoreNode::Mul(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Consider(s("in_attack_range")),
        ]);
        let values = consideration_values(&[(s("has_target"), 0.8), (s("in_attack_range"), 0.5)]);
        assert!((node.evaluate(&values) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_and_short_circuits_on_zero() {
        let node = ScoreNode::Mul(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Consider(s("in_attack_range")),
        ]);
        let values = consideration_values(&[(s("has_target"), 0.0), (s("in_attack_range"), 1.0)]);
        assert!((node.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_or_node() {
        let node = ScoreNode::Max(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Consider(s("in_attack_range")),
        ]);
        let values = consideration_values(&[(s("has_target"), 0.3), (s("in_attack_range"), 0.7)]);
        assert!((node.evaluate(&values) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_not_node() {
        let node = ScoreNode::Not(Box::new(ScoreNode::Consider(s("has_target"))));
        let values = consideration_values(&[(s("has_target"), 0.3)]);
        assert!((node.evaluate(&values) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_not_clamps_input() {
        let node = ScoreNode::Not(Box::new(ScoreNode::Fixed(1.5)));
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_average_node() {
        let node = ScoreNode::Average(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Consider(s("in_attack_range")),
        ]);
        let values = consideration_values(&[(s("has_target"), 0.3), (s("in_attack_range"), 0.5)]);
        assert!((node.evaluate(&values) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fixed_node() {
        let node = ScoreNode::Fixed(0.42);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 0.42).abs() < f32::EPSILON);
    }

    #[test]
    fn test_weighted_node() {
        let node = ScoreNode::Weighted(0.5, Box::new(ScoreNode::Consider(s("has_target"))));
        let values = consideration_values(&[(s("has_target"), 0.8)]);
        assert!((node.evaluate(&values) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_nested_tree() {
        let node = ScoreNode::Mul(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Not(Box::new(ScoreNode::Consider(s("in_attack_range")))),
        ]);
        let values = consideration_values(&[(s("has_target"), 1.0), (s("in_attack_range"), 0.3)]);
        assert!((node.evaluate(&values) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_complex_tree() {
        let node = ScoreNode::Mul(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Not(Box::new(ScoreNode::Consider(s("in_attack_range")))),
            ScoreNode::Max(vec![
                ScoreNode::Consider(s("target_locked")),
                ScoreNode::Consider(s("target_visible")),
            ]),
        ]);

        let values = consideration_values(&[
            (s("has_target"), 1.0),
            (s("in_attack_range"), 0.0),
            (s("target_locked"), 0.0),
            (s("target_visible"), 0.8),
        ]);

        assert!((node.evaluate(&values) - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_evaluate_profile_first_to_score() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![],
            behaviors: vec![
                BehaviorDef {
                    action: a("AttackPrimary"),
                    score: ScoreNode::Consider(s("in_attack_range")),
                },
                BehaviorDef {
                    action: a("Chase"),
                    score: ScoreNode::Consider(s("has_target")),
                },
                BehaviorDef {
                    action: a("Idle"),
                    score: ScoreNode::Fixed(0.5),
                },
            ],
        };

        let values = consideration_values(&[(s("in_attack_range"), 0.8), (s("has_target"), 0.9)]);

        let result = evaluate_profile(&profile, &values);
        assert_eq!(result.action, Some(a("AttackPrimary")));
        assert_eq!(result.winner, Some(0));
    }

    #[test]
    fn test_evaluate_profile_falls_back_to_occupation() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![],
            behaviors: vec![
                BehaviorDef {
                    action: a("AttackPrimary"),
                    score: ScoreNode::Consider(s("in_attack_range")),
                },
                BehaviorDef {
                    action: a("Idle"),
                    score: ScoreNode::Fixed(0.5),
                },
            ],
        };

        let values = consideration_values(&[(s("in_attack_range"), 0.2)]);

        let result = evaluate_profile(&profile, &values);
        assert_eq!(result.action, Some(a("Idle")));
        assert_eq!(result.winner, Some(1));
    }

    #[test]
    fn test_evaluate_profile_no_winner() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![],
            behaviors: vec![BehaviorDef {
                action: a("AttackPrimary"),
                score: ScoreNode::Consider(s("in_attack_range")),
            }],
        };

        let values = consideration_values(&[(s("in_attack_range"), 0.2)]);

        let result = evaluate_profile(&profile, &values);
        assert_eq!(result.action, None);
        assert_eq!(result.winner, None);
    }

    #[test]
    fn test_evaluate_profile_weighted_node() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![],
            behaviors: vec![
                BehaviorDef {
                    action: a("Chase"),
                    score: ScoreNode::Weighted(0.5, Box::new(ScoreNode::Fixed(0.9))),
                },
                BehaviorDef {
                    action: a("Idle"),
                    score: ScoreNode::Fixed(0.5),
                },
            ],
        };

        let values = consideration_values(&[]);
        let result = evaluate_profile(&profile, &values);
        // Chase scores 0.9 * 0.5 = 0.45 (below threshold), Idle scores 0.5 (at threshold)
        assert_eq!(result.action, Some(a("Idle")));
    }

    #[test]
    fn test_referenced_considerations() {
        let tree = ScoreNode::Mul(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Max(vec![
                ScoreNode::Consider(s("can_see_player")),
                ScoreNode::Not(Box::new(ScoreNode::Consider(s("in_attack_range")))),
            ]),
            ScoreNode::Fixed(0.5),
            ScoreNode::Weighted(0.8, Box::new(ScoreNode::Consider(s("has_target")))),
        ]);

        let ids = tree.referenced_considerations();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&s("has_target")));
        assert!(ids.contains(&s("can_see_player")));
        assert!(ids.contains(&s("in_attack_range")));
    }

    #[test]
    fn test_ron_roundtrip_score_node() {
        let tree = ScoreNode::Mul(vec![
            ScoreNode::Consider(s("has_target")),
            ScoreNode::Not(Box::new(ScoreNode::Consider(s("in_attack_range")))),
            ScoreNode::Max(vec![
                ScoreNode::Consider(s("target_visible")),
                ScoreNode::Weighted(0.7, Box::new(ScoreNode::Consider(s("target_memorized")))),
            ]),
        ]);

        let ron_str = ron::ser::to_string_pretty(&tree, ron::ser::PrettyConfig::default()).unwrap();
        let deserialized: ScoreNode = ron::from_str(&ron_str).unwrap();
        assert_eq!(tree, deserialized);
    }

    #[test]
    fn test_momentum_as_consideration() {
        // Momentum is just another consideration ID — the game sets "momentum:Chase" to 1.0
        // when the actor is currently in Chase state
        let chase_tree = ScoreNode::Average(vec![
            ScoreNode::Weighted(0.8, Box::new(ScoreNode::Consider(s("has_target")))),
            ScoreNode::Weighted(0.2, Box::new(ScoreNode::Consider(s("momentum:Chase")))),
        ]);

        let values = consideration_values(&[(s("has_target"), 1.0), (s("momentum:Chase"), 1.0)]);
        assert!((chase_tree.evaluate(&values) - 0.5).abs() < f32::EPSILON);

        let values = consideration_values(&[(s("has_target"), 1.0), (s("momentum:Chase"), 0.0)]);
        assert!((chase_tree.evaluate(&values) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_and_evaluates_to_one() {
        let node = ScoreNode::Mul(vec![]);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_or_evaluates_to_zero() {
        let node = ScoreNode::Max(vec![]);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_or_short_circuits_on_one() {
        let node = ScoreNode::Max(vec![
            ScoreNode::Fixed(1.0),
            ScoreNode::Consider(s("never_read")),
        ]);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_curve_node_exponential() {
        let curve = ResponseCurve {
            curve_type: CurveType::Exponential,
            m: 1.0,
            k: 2.0,
            b: 0.0,
            c: 0.0,
        };
        let node = ScoreNode::Curve(curve, Box::new(ScoreNode::Fixed(0.5)));
        let values = consideration_values(&[]);
        let score = node.evaluate(&values);
        // x^2 at 0.5 = 0.25
        assert!((score - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_curve_clamps_output() {
        let curve = ResponseCurve::default();
        let node = ScoreNode::Curve(curve, Box::new(ScoreNode::Fixed(1.5)));
        let values = consideration_values(&[]);
        let score = node.evaluate(&values);
        assert!((score - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_clamp_node() {
        let node = ScoreNode::Fixed(1.5).clamped(0.0, 1.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_clamp_node_lower_bound() {
        let node = ScoreNode::Fixed(-0.5).clamped(0.0, 1.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_clamp_node_passthrough() {
        let node = ScoreNode::Fixed(0.5).clamped(0.0, 1.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_clamp_custom_range() {
        let node = ScoreNode::Fixed(0.8).clamped(0.2, 0.6);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_node() {
        let node = ScoreNode::Fixed(75.0).normalized(50.0, 100.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_at_min() {
        let node = ScoreNode::Fixed(50.0).normalized(50.0, 100.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_at_max() {
        let node = ScoreNode::Fixed(100.0).normalized(50.0, 100.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_below_min_clamps() {
        let node = ScoreNode::Fixed(10.0).normalized(50.0, 100.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_above_max_clamps() {
        let node = ScoreNode::Fixed(200.0).normalized(50.0, 100.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_zero_range() {
        let node = ScoreNode::Fixed(5.0).normalized(5.0, 5.0);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 1.0).abs() < f32::EPSILON);

        let below = ScoreNode::Fixed(4.0).normalized(5.0, 5.0);
        assert!((below.evaluate(&values)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_clamp_collects_considerations() {
        let node = ScoreNode::Consider(s("health")).clamped(0.0, 1.0);
        let ids = node.referenced_considerations();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&s("health")));
    }

    #[test]
    fn test_normalize_collects_considerations() {
        let node = ScoreNode::Consider(s("distance")).normalized(0.0, 100.0);
        let ids = node.referenced_considerations();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&s("distance")));
    }

    #[test]
    fn test_ron_roundtrip_clamp_normalize() {
        let tree = ScoreNode::Mul(vec![
            ScoreNode::Consider(s("health")).clamped(0.0, 1.0),
            ScoreNode::Consider(s("distance")).normalized(10.0, 200.0),
        ]);

        let ron_str = ron::ser::to_string_pretty(&tree, ron::ser::PrettyConfig::default()).unwrap();
        let deserialized: ScoreNode = ron::from_str(&ron_str).unwrap();
        assert_eq!(tree, deserialized);
    }

    #[test]
    fn test_curve_collects_considerations() {
        let node = ScoreNode::Curve(
            ResponseCurve::default(),
            Box::new(ScoreNode::Consider(s("health"))),
        );
        let ids = node.referenced_considerations();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&s("health")));
    }

    // ── Composite consideration tests ────────────────────────────────────

    #[test]
    fn test_resolve_composites_basic() {
        let composites = vec![CompositeConsiderationDef {
            id: s("target_detected"),
            score: ScoreNode::Max(vec![
                ScoreNode::Consider(s("can_see")),
                ScoreNode::Consider(s("can_hear")),
            ]),
        }];

        let mut values = consideration_values(&[(s("can_see"), 0.0), (s("can_hear"), 0.8)]);
        resolve_composites(&composites, &mut values).unwrap();
        assert!((values[&s("target_detected")] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resolve_composites_chain() {
        // B depends on A
        let composites = vec![
            CompositeConsiderationDef {
                id: s("a"),
                score: ScoreNode::Consider(s("raw")),
            },
            CompositeConsiderationDef {
                id: s("b"),
                score: ScoreNode::Mul(vec![ScoreNode::Consider(s("a")), ScoreNode::Fixed(0.5)]),
            },
        ];

        let mut values = consideration_values(&[(s("raw"), 0.8)]);
        resolve_composites(&composites, &mut values).unwrap();
        assert!((values[&s("a")] - 0.8).abs() < f32::EPSILON);
        assert!((values[&s("b")] - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resolve_composites_reverse_order() {
        // Defined in reverse dependency order — topological sort should handle it
        let composites = vec![
            CompositeConsiderationDef {
                id: s("b"),
                score: ScoreNode::Consider(s("a")),
            },
            CompositeConsiderationDef {
                id: s("a"),
                score: ScoreNode::Consider(s("raw")),
            },
        ];

        let mut values = consideration_values(&[(s("raw"), 0.7)]);
        resolve_composites(&composites, &mut values).unwrap();
        assert!((values[&s("a")] - 0.7).abs() < f32::EPSILON);
        assert!((values[&s("b")] - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resolve_composites_cycle_detection() {
        let composites = vec![
            CompositeConsiderationDef {
                id: s("a"),
                score: ScoreNode::Consider(s("b")),
            },
            CompositeConsiderationDef {
                id: s("b"),
                score: ScoreNode::Consider(s("a")),
            },
        ];

        let mut values = consideration_values(&[]);
        let result = resolve_composites(&composites, &mut values);
        assert!(result.is_err());
        match result.unwrap_err() {
            CompositeError::Cycle(ids) => {
                assert!(ids.contains(&s("a")));
                assert!(ids.contains(&s("b")));
            }
        }
    }

    #[test]
    fn test_resolve_composites_self_reference() {
        let composites = vec![CompositeConsiderationDef {
            id: s("a"),
            score: ScoreNode::Consider(s("a")),
        }];

        let mut values = consideration_values(&[]);
        let result = resolve_composites(&composites, &mut values);
        assert!(matches!(result, Err(CompositeError::Cycle(_))));
    }

    #[test]
    fn test_resolve_composites_empty() {
        let mut values = consideration_values(&[(s("raw"), 1.0)]);
        resolve_composites(&[], &mut values).unwrap();
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn test_composite_in_behavior_tree() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![CompositeConsiderationDef {
                id: s("threat"),
                score: ScoreNode::Max(vec![
                    ScoreNode::Consider(s("can_see")),
                    ScoreNode::Consider(s("can_hear")),
                ]),
            }],
            behaviors: vec![
                BehaviorDef {
                    action: a("Chase"),
                    score: ScoreNode::Consider(s("threat")),
                },
                BehaviorDef {
                    action: a("Idle"),
                    score: ScoreNode::Fixed(0.5),
                },
            ],
        };

        let values = consideration_values(&[(s("can_see"), 0.0), (s("can_hear"), 0.9)]);
        let result = evaluate_profile(&profile, &values);
        assert_eq!(result.action, Some(a("Chase")));
    }

    #[test]
    fn test_composite_overrides_raw_consideration() {
        // If a composite has the same ID as a raw consideration, it overwrites it
        let composites = vec![CompositeConsiderationDef {
            id: s("health"),
            score: ScoreNode::Fixed(0.0),
        }];

        let mut values = consideration_values(&[(s("health"), 1.0)]);
        resolve_composites(&composites, &mut values).unwrap();
        assert!((values[&s("health")]).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ron_roundtrip_with_composites() {
        let profile = UtilityAIProfile {
            threshold: 0.5,
            strategy: PickStrategy::default(),
            composites: vec![CompositeConsiderationDef {
                id: s("threat"),
                score: ScoreNode::Max(vec![
                    ScoreNode::Consider(s("can_see")),
                    ScoreNode::Consider(s("can_hear")),
                ]),
            }],
            behaviors: vec![BehaviorDef {
                action: a("Chase"),
                score: ScoreNode::Consider(s("threat")),
            }],
        };

        let ron_str =
            ron::ser::to_string_pretty(&profile, ron::ser::PrettyConfig::default()).unwrap();
        let deserialized: UtilityAIProfile = ron::from_str(&ron_str).unwrap();
        assert_eq!(deserialized.composites.len(), 1);
        assert_eq!(deserialized.behaviors.len(), 1);
    }

    #[test]
    fn test_ron_deserialize_without_composites() {
        // Existing RON without composites field should still work
        let ron_str = r#"(
            threshold: 0.5,
            behaviors: [
                (
                    action: "Idle",
                    score: Fixed(0.5),
                ),
            ],
        )"#;
        let profile: UtilityAIProfile = ron::from_str(ron_str).unwrap();
        assert!(profile.composites.is_empty());
        assert_eq!(profile.behaviors.len(), 1);
    }

    #[test]
    fn test_composite_error_display() {
        let err = CompositeError::Cycle(vec![s("a"), s("b")]);
        let msg = format!("{err}");
        assert!(msg.contains("Cycle"));
        assert!(msg.contains("a"));
        assert!(msg.contains("b"));
    }

    #[test]
    fn test_three_level_composite_chain() {
        let composites = vec![
            CompositeConsiderationDef {
                id: s("level1"),
                score: ScoreNode::Consider(s("raw")),
            },
            CompositeConsiderationDef {
                id: s("level2"),
                score: ScoreNode::Consider(s("level1")),
            },
            CompositeConsiderationDef {
                id: s("level3"),
                score: ScoreNode::Consider(s("level2")),
            },
        ];

        let mut values = consideration_values(&[(s("raw"), 0.6)]);
        resolve_composites(&composites, &mut values).unwrap();
        assert!((values[&s("level3")] - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_geom_node_two_values() {
        let node = ScoreNode::Geom(vec![
            ScoreNode::Consider(s("a")),
            ScoreNode::Consider(s("b")),
        ]);
        let values = consideration_values(&[(s("a"), 0.25), (s("b"), 0.64)]);
        // geom(0.25, 0.64) = sqrt(0.25 * 0.64) = sqrt(0.16) = 0.4
        assert!((node.evaluate(&values) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_geom_evaluates_to_one() {
        let node = ScoreNode::Geom(vec![]);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_geom_three_equal_values() {
        let node = ScoreNode::Geom(vec![
            ScoreNode::Fixed(0.5),
            ScoreNode::Fixed(0.5),
            ScoreNode::Fixed(0.5),
        ]);
        let values = consideration_values(&[]);
        // geom of equal values is the value itself
        assert!((node.evaluate(&values) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_geom_single_value() {
        let node = ScoreNode::Geom(vec![ScoreNode::Fixed(0.7)]);
        let values = consideration_values(&[]);
        assert!((node.evaluate(&values) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_geom_collects_considerations() {
        let node = ScoreNode::Geom(vec![
            ScoreNode::Consider(s("health")),
            ScoreNode::Consider(s("energy")),
        ]);
        let ids = node.referenced_considerations();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&s("health")));
        assert!(ids.contains(&s("energy")));
    }

    #[test]
    fn test_geom_ron_roundtrip() {
        let tree = ScoreNode::Geom(vec![
            ScoreNode::Consider(s("health")),
            ScoreNode::Consider(s("energy")),
        ]);
        let ron_str = ron::ser::to_string_pretty(&tree, ron::ser::PrettyConfig::default()).unwrap();
        let deserialized: ScoreNode = ron::from_str(&ron_str).unwrap();
        assert_eq!(tree, deserialized);
    }
}
