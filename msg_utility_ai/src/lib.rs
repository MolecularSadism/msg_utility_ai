//! Utility AI: pick an action by scoring every option and taking a winner.
//!
//! An agent holds a [`UtilityAIProfile`] asset (loaded from a `.ai.ron` file) listing
//! [`BehaviorDef`]s. Each behavior pairs an [`ActionId`] with a [`ScoreNode`] expression tree that
//! evaluates to a number. Leaves of that tree are [`ConsiderationId`] lookups into a per-entity
//! [`Consideration`] map; the branches combine them (`Mul`, `Max`, `Average`, `Geom`, `Not`,
//! `Weighted`, response `Curve`s, `Clamp`, `Normalize`). Every behavior is scored against the same
//! consideration values, then the profile's [`PickStrategy`] picks among those clearing the
//! profile's `threshold` — either the first in list order or the highest. The winner lands in
//! [`CurrentAction`]; nothing clearing the threshold leaves it `None`.
//!
//! The library knows nothing about the game: considerations and actions are opaque interned
//! strings. The game writes sensor readings into [`Consideration`] and maps the resulting
//! [`ActionId`] onto whatever it uses to act (in Gravitaria, FSM states — see
//! `src/actors/utility/`).
//!
//! Considerations are conventionally normalized to `0.0..=1.0`; missing ones read as `0.0`.
//! [`Observable`] exists so sensors can write `bool` as well as floats.
//! [`CompositeConsiderationDef`] names a reusable subexpression that is resolved into the
//! consideration map before behaviors are scored, so several behaviors can share one piece of
//! reasoning.
//!
//! Running it: [`UtilityAiPlugin`] owns a timer and its own [`UtilityAiSchedule`], and splits
//! agents into phase buckets so each run re-evaluates only a fraction of the population — see the
//! [`stagger`] module. Apps with their own AI schedule use [`AiStaggerPlugin`] plus
//! [`stagger_agents`] and [`evaluate_agents`] directly.

pub mod components;
mod config;
mod observable;
pub mod picker;
pub mod plugin;
pub mod scoring;
pub mod stagger;

pub use components::{AiProfile, Consideration, CurrentAction};
pub use config::{
    ActionId, BehaviorDef, CompositeConsiderationDef, ConsiderationId, DefaultOccupation,
    UtilityAIProfile,
};
pub use observable::Observable;
pub use picker::{PickStrategy, pick};
pub use plugin::{
    DEFAULT_AI_INTERVAL, UtilityAiPlugin, UtilityAiSchedule, UtilityAiSystems, evaluate_agents,
};
pub use scoring::{
    CompositeError, CurveType, EvaluationResult, ResponseCurve, ScoreNode, evaluate_profile,
};
pub use stagger::{AiPhase, AiStaggerPlugin, CurrentAiPhase, DEFAULT_AI_PHASES, stagger_agents};
