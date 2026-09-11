//! Bevy wiring: a timer-driven schedule that evaluates one phase bucket of
//! agents per run.

use std::time::Duration;

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use crate::components::{AiProfile, Consideration, CurrentAction};
use crate::config::UtilityAIProfile;
use crate::picker::pick_action;
use crate::stagger::{AiPhase, AiStaggerPlugin, CurrentAiPhase, DEFAULT_AI_PHASES, stagger_agents};

pub use crate::stagger::DEFAULT_AI_INTERVAL;

/// Schedule [`UtilityAiPlugin`] runs its staggered AI systems in.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UtilityAiSchedule;

/// Opt-in Bevy plugin that evaluates AI agents on a timer, spread across frames.
///
/// Agents are split into `phases` buckets. [`UtilityAiSchedule`] runs once per
/// [`AiStaggerPlugin::phase_interval`] and each run evaluates one bucket, so an
/// agent rethinks once per `interval` while only a fraction of the population
/// is evaluated on any single run.
///
/// The game should populate [`Consideration`] components (via [`Consideration::set`])
/// before [`UtilityAiSystems::Evaluate`] runs — schedule those systems in
/// [`UtilityAiSchedule`] under [`UtilityAiSystems::Prepare`] so they stay on the same
/// cadence. After evaluation, [`CurrentAction`] holds the winning action for each
/// entity.
///
/// Apps with their own AI schedule should skip this plugin, add
/// [`AiStaggerPlugin`] for the components and the phase resource, and schedule
/// [`stagger_agents`] and [`evaluate_agents`] themselves.
///
/// ```
/// use std::time::Duration;
///
/// use bevy::prelude::*;
/// use msg_utility_ai::UtilityAiPlugin;
///
/// let mut app = App::new();
/// app.add_plugins(MinimalPlugins);
/// app.add_plugins(UtilityAiPlugin::new(Duration::from_millis(200), 8));
/// app.update();
/// ```
pub struct UtilityAiPlugin {
    interval: Duration,
    phases: u8,
}

impl Default for UtilityAiPlugin {
    fn default() -> Self {
        Self {
            interval: DEFAULT_AI_INTERVAL,
            phases: DEFAULT_AI_PHASES,
        }
    }
}

impl UtilityAiPlugin {
    /// Evaluate each agent once per `interval`, spread over `phases` buckets.
    ///
    /// A phase count of zero is clamped to one, which evaluates every agent on
    /// every run.
    pub fn new(interval: Duration, phases: u8) -> Self {
        Self {
            interval,
            phases: phases.max(1),
        }
    }

    /// The [`AiStaggerPlugin`] this plugin builds on, owning the phase-interval arithmetic.
    fn stagger_plugin(&self) -> AiStaggerPlugin {
        AiStaggerPlugin {
            interval: self.interval,
            phases: self.phases,
        }
    }
}

impl Plugin for UtilityAiPlugin {
    fn build(&self, app: &mut App) {
        let stagger_plugin = self.stagger_plugin();
        app.insert_resource(AiPhaseTimer(Timer::new(
            stagger_plugin.phase_interval(),
            TimerMode::Repeating,
        )));
        app.add_plugins(stagger_plugin);

        app.add_schedule(Schedule::new(UtilityAiSchedule));
        app.configure_sets(
            UtilityAiSchedule,
            (UtilityAiSystems::Prepare, UtilityAiSystems::Evaluate).chain(),
        );
        app.add_systems(
            UtilityAiSchedule,
            (
                stagger_agents.in_set(UtilityAiSystems::Prepare),
                evaluate_agents::<()>.in_set(UtilityAiSystems::Evaluate),
            ),
        );
        app.add_systems(Update, run_utility_ai_schedule);
    }
}

/// Counts down one phase slice of the rethink interval.
#[derive(Resource, Debug)]
struct AiPhaseTimer(Timer);

/// Runs [`UtilityAiSchedule`] once per phase slice.
fn run_utility_ai_schedule(world: &mut World) {
    let Some(delta) = world.get_resource::<Time>().map(|time| time.delta()) else {
        return;
    };

    let due = world
        .get_resource_mut::<AiPhaseTimer>()
        .is_some_and(|mut timer| {
            timer.0.tick(delta);
            timer.0.just_finished()
        });

    if due {
        let _ = world.try_run_schedule(UtilityAiSchedule);
    }
}

/// System sets for AI evaluation ordering.
///
/// Schedule consideration updates in [`UtilityAiSystems::Prepare`] so values are
/// fresh when [`UtilityAiSystems::Evaluate`] runs.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UtilityAiSystems {
    /// Rotate the AI phase and mark the agents due this run.
    Prepare,
    /// Evaluate AI profiles and write the winning action.
    Evaluate,
}

/// Evaluates due AI entities, running `pick` on each and writing the result to `CurrentAction`.
///
/// Only agents whose [`AiPhase`] matches [`CurrentAiPhase`] this run are
/// evaluated, spreading the population's cost across the staggering cadence.
///
/// The game can schedule this system directly instead of using [`UtilityAiPlugin`]
/// when custom scheduling is needed (e.g. a slower timer-gated schedule).
///
/// The `F` query filter lets callers further restrict evaluation to a subset
/// of entities per run, with `()` meaning "no extra restriction".
pub fn evaluate_agents<F: bevy::ecs::query::QueryFilter + 'static>(
    mut query: Query<(&Consideration, &AiProfile, &AiPhase, &mut CurrentAction), F>,
    current: Res<CurrentAiPhase>,
    profiles: Res<Assets<UtilityAIProfile>>,
) {
    for (considerations, ai_profile, phase, mut current_action) in &mut query {
        if !phase.is_due(&current) {
            continue;
        }
        let Some(profile) = profiles.get(&ai_profile.0) else {
            continue;
        };
        current_action.0 = pick_action(profile, &considerations.0);
    }
}
