//! Spreading AI evaluation across frames.
//!
//! Evaluating every agent on the same run turns AI cost into a periodic spike:
//! the frame a rethink lands on pays for the whole population while the frames
//! between it do nothing. Staggering trades that spike for a flat cost by
//! splitting agents into phase buckets and evaluating one bucket per run.
//!
//! Each agent is assigned an [`AiPhase`] round-robin the first time
//! [`stagger_agents`] sees it. Every run advances [`CurrentAiPhase`], so a
//! schedule ticking once per [`AiStaggerPlugin::phase_interval`] still gives
//! every agent one rethink per `interval` while only `1 / phases` of them are
//! due at a time.
//!
//! Sensor and behavior systems opt in by comparing their own [`AiPhase`]
//! against [`CurrentAiPhase::phase`] (see [`AiPhase::is_due`]), which keeps
//! their cost on the same cadence as evaluation without an insert/remove
//! marker component. A single [`AiStaggerPlugin::phase_interval`] is the
//! cadence every AI-adjacent subsystem ticks on — in Gravitaria, `utility`
//! runs its slow decision schedule on it, `targeting` re-aims its vision
//! raycasters on it, and `perception` rebuilds its memory digest on it.
//! Depending on one shared, derived interval keeps those subsystems'
//! work synchronized without introducing a dependency cycle between them.

use std::time::Duration;

use bevy::prelude::*;

use crate::components::CurrentAction;

/// Number of phase buckets used when none is configured.
pub const DEFAULT_AI_PHASES: u8 = 4;

/// Total rethink interval used when none is configured.
pub const DEFAULT_AI_INTERVAL: Duration = Duration::from_millis(100);

/// The phase bucket an AI agent belongs to. Assigned round-robin on first sight.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct AiPhase(pub u8);

impl AiPhase {
    /// Whether this agent's phase is the one due for evaluation this run.
    pub fn is_due(&self, current: &CurrentAiPhase) -> bool {
        self.0 == current.phase
    }
}

/// Rotating phase counter plus the round-robin cursor for newly seen agents.
#[derive(Resource, Reflect, Debug, Clone, Copy)]
#[reflect(Resource)]
pub struct CurrentAiPhase {
    /// The bucket the most recent [`stagger_agents`] run made due.
    pub phase: u8,
    /// Number of buckets agents are spread across.
    phases: u8,
    /// Cursor handing out buckets to agents that do not have one yet.
    next_assignment: u8,
}

impl Default for CurrentAiPhase {
    fn default() -> Self {
        Self::new(DEFAULT_AI_PHASES)
    }
}

impl CurrentAiPhase {
    /// A rotation over `phases` buckets.
    ///
    /// A count of zero is clamped to one, which puts every agent in phase `0`
    /// and keeps `phase` pinned there — every agent is due on every run,
    /// staggering off, without a division by zero.
    pub fn new(phases: u8) -> Self {
        Self {
            phase: 0,
            phases: phases.max(1),
            next_assignment: 0,
        }
    }

    /// Number of buckets agents are spread across.
    pub fn phases(&self) -> u8 {
        self.phases
    }
}

/// Advance the AI phase to the bucket due this run.
///
/// Agents holding a [`CurrentAction`] but no [`AiPhase`] are assigned one
/// round-robin, so load stays evenly distributed as the population changes.
pub fn stagger_agents(
    mut commands: Commands,
    mut current: ResMut<CurrentAiPhase>,
    q_unassigned: Query<Entity, (With<CurrentAction>, Without<AiPhase>)>,
) {
    let phases = current.phases;

    for entity in &q_unassigned {
        commands
            .entity(entity)
            .insert(AiPhase(current.next_assignment));
        current.next_assignment = (current.next_assignment + 1) % phases;
    }

    current.phase = (current.phase + 1) % phases;
}

/// Registers the staggering types and the rotating phase resource without
/// scheduling anything.
///
/// [`UtilityAiPlugin`](crate::UtilityAiPlugin) adds this itself. Add it directly
/// when the app drives AI from its own schedule and only needs the components
/// and the resource, then schedule [`stagger_agents`] and
/// [`evaluate_agents`](crate::evaluate_agents) where they belong.
pub struct AiStaggerPlugin {
    /// Total time each agent should take to rethink once, across every phase bucket.
    pub interval: Duration,
    /// Number of phase buckets agents are spread across.
    pub phases: u8,
}

impl AiStaggerPlugin {
    /// One phase slice of [`interval`](Self::interval): how often a schedule carrying this
    /// plugin's phases should run so each agent rethinks once per `interval` while only
    /// `1 / phases` of the population is due on any single run. Floors at one millisecond so a
    /// short interval or a high phase count never produces a zero-duration timer.
    pub fn phase_interval(&self) -> Duration {
        (self.interval / u32::from(self.phases.max(1))).max(Duration::from_millis(1))
    }
}

impl Default for AiStaggerPlugin {
    fn default() -> Self {
        Self {
            interval: DEFAULT_AI_INTERVAL,
            phases: DEFAULT_AI_PHASES,
        }
    }
}

impl Plugin for AiStaggerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<AiPhase>();
        app.register_type::<CurrentAiPhase>();
        app.insert_resource(CurrentAiPhase::new(self.phases));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHASES: u8 = 4;

    fn make_app() -> App {
        let mut app = App::new();
        app.insert_resource(CurrentAiPhase::new(PHASES));
        app.add_systems(Update, stagger_agents);
        app
    }

    fn spawn_agents(app: &mut App, count: usize) -> Vec<Entity> {
        (0..count)
            .map(|_| app.world_mut().spawn(CurrentAction(None)).id())
            .collect()
    }

    #[test]
    fn agents_get_round_robin_phases() {
        let mut app = make_app();
        let entities = spawn_agents(&mut app, 8);

        app.update();

        // Commands apply within the run; phases are visible afterwards.
        let mut counts = [0usize; PHASES as usize];
        for entity in &entities {
            let phase = app.world().entity(*entity).get::<AiPhase>().unwrap();
            counts[phase.0 as usize] += 1;
        }
        assert_eq!(counts, [2, 2, 2, 2], "phases should be evenly distributed");
    }

    #[test]
    fn each_agent_is_due_once_per_full_cycle() {
        let mut app = make_app();
        let entities = spawn_agents(&mut app, 4);

        // First run assigns phases (phases are visible next run).
        app.update();

        let mut due_counts = vec![0usize; entities.len()];
        for _ in 0..PHASES {
            app.update();
            let current = *app.world().resource::<CurrentAiPhase>();
            for (i, entity) in entities.iter().enumerate() {
                let phase = app.world().entity(*entity).get::<AiPhase>().unwrap();
                if phase.is_due(&current) {
                    due_counts[i] += 1;
                }
            }
        }
        assert_eq!(
            due_counts,
            vec![1; entities.len()],
            "every agent must be due exactly once per full phase cycle"
        );
    }

    #[test]
    fn phase_interval_is_one_slice_of_the_rethink_interval() {
        let plugin = AiStaggerPlugin {
            interval: Duration::from_millis(800),
            phases: 4,
        };
        assert_eq!(plugin.phase_interval(), Duration::from_millis(200));
    }

    #[test]
    fn phase_interval_floors_at_one_millisecond() {
        let plugin = AiStaggerPlugin {
            interval: Duration::from_millis(2),
            phases: 4,
        };
        assert_eq!(plugin.phase_interval(), Duration::from_millis(1));
    }

    #[test]
    fn phase_interval_treats_zero_phases_as_one() {
        let plugin = AiStaggerPlugin {
            interval: Duration::from_millis(100),
            phases: 0,
        };
        assert_eq!(plugin.phase_interval(), Duration::from_millis(100));
    }

    #[test]
    fn a_single_phase_marks_every_agent_every_run() {
        let mut app = App::new();
        app.insert_resource(CurrentAiPhase::new(0));
        app.add_systems(Update, stagger_agents);
        let entities = spawn_agents(&mut app, 3);

        assert_eq!(app.world().resource::<CurrentAiPhase>().phases(), 1);

        app.update();
        app.update();

        let current = *app.world().resource::<CurrentAiPhase>();
        for entity in &entities {
            let phase = app.world().entity(*entity).get::<AiPhase>().unwrap();
            assert!(phase.is_due(&current));
        }
    }
}
