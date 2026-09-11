//! Sensor input conversion, so a consideration can be written as a `bool` or a
//! float without the call site spelling out the cast.

use std::ops::Deref;

/// A sensor input value that can be constructed from `bool`, `f32`, or `f64`.
///
/// `Observable` is a thin newtype over `f32` that makes boolean sensor
/// values ergonomic to express. `true` maps to `1.0`, `false` maps to `0.0`.
///
/// ```
/// use msg_utility_ai::Observable;
///
/// let from_bool: Observable = true.into();
/// assert_eq!(*from_bool, 1.0);
///
/// let from_float: Observable = 0.75f32.into();
/// assert_eq!(*from_float, 0.75);
///
/// let from_f64: Observable = 0.5f64.into();
/// assert_eq!(*from_f64, 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Observable(f32);

impl Observable {
    /// Wrap an already-normalized float.
    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    /// The wrapped float.
    pub const fn value(self) -> f32 {
        self.0
    }
}

impl From<bool> for Observable {
    fn from(value: bool) -> Self {
        Self(if value { 1.0 } else { 0.0 })
    }
}

impl From<f32> for Observable {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<f64> for Observable {
    fn from(value: f64) -> Self {
        Self(value as f32)
    }
}

impl Deref for Observable {
    type Target = f32;

    fn deref(&self) -> &f32 {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_true_maps_to_one() {
        let obs: Observable = true.into();
        assert_eq!(*obs, 1.0);
    }

    #[test]
    fn bool_false_maps_to_zero() {
        let obs: Observable = false.into();
        assert_eq!(*obs, 0.0);
    }

    #[test]
    fn f32_passthrough() {
        let obs: Observable = 0.42f32.into();
        assert_eq!(*obs, 0.42);
    }

    #[test]
    fn f64_truncates_to_f32() {
        let obs: Observable = 0.5f64.into();
        assert_eq!(*obs, 0.5);
    }

    #[test]
    fn deref_allows_arithmetic() {
        let obs: Observable = 0.5f32.into();
        let result = *obs * 2.0;
        assert!((result - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn value_method() {
        let obs = Observable::new(0.75);
        assert_eq!(obs.value(), 0.75);
    }
}
