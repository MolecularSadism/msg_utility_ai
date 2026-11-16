//! The three components an AI agent carries: its sensor readings, its profile
//! handle, and the action evaluation picked for it.

use bevy::platform::collections::HashMap;

use bevy::prelude::*;

use crate::config::{ActionId, ConsiderationId, UtilityAIProfile};

/// Per-entity consideration values that feed into the scoring tree.
///
/// The game populates this map each tick before the AI evaluation runs.
/// Keys are consideration IDs, values are floats in the 0.0-1.0 range.
///
/// Use [`Consideration::set`] to insert values with automatic [`Observable`](crate::Observable)
/// conversion — accepts `bool`, `f32`, or `f64`.
#[derive(Component, Reflect, Default, Debug, Clone, Deref, DerefMut)]
#[reflect(Component, Default, Debug, Clone)]
pub struct Consideration(pub HashMap<ConsiderationId, f32>);

impl Consideration {
    /// Insert a consideration value, accepting anything convertible to
    /// [`Observable`](crate::Observable).
    ///
    /// This allows passing `bool`, `f32`, or `f64` values directly.
    pub fn set(&mut self, id: ConsiderationId, value: impl Into<crate::Observable>) {
        self.0.insert(id, value.into().value());
    }
}

/// The action selected by the AI evaluation, or `None` if no behavior won.
///
/// Updated each tick by the plugin's evaluation system.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct CurrentAction(pub Option<ActionId>);

/// Handle to a loaded [`UtilityAIProfile`] asset.
#[derive(Component, Reflect, Debug, Clone, Deref)]
#[reflect(Component)]
pub struct AiProfile(pub Handle<UtilityAIProfile>);

#[cfg(test)]
mod tests {
    use bevy::reflect::FromReflect;

    use super::*;

    #[test]
    fn consideration_rebuilds_from_reflection() {
        let mut consideration = Consideration::default();
        consideration.set(ConsiderationId::new("has_target"), true);

        let rebuilt = Consideration::from_reflect(consideration.as_partial_reflect())
            .expect("Consideration rebuilds from its reflection");
        assert_eq!(rebuilt.0, consideration.0);
    }
}
