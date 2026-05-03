use std::collections::HashMap;

use hs_device_contracts::StateMessage;
use serde_json::Value;

pub struct StateFilter {
    last_states: HashMap<String, Value>,
    numeric_thresholds: HashMap<String, f64>,
}

impl Default for StateFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl StateFilter {
    pub fn new() -> Self {
        Self {
            last_states: HashMap::new(),
            numeric_thresholds: HashMap::new(),
        }
    }

    pub fn with_numeric_thresholds(thresholds: &[(&str, f64)]) -> Self {
        let mut filter = Self::new();
        for (capability_id, threshold) in thresholds {
            filter
                .numeric_thresholds
                .insert((*capability_id).to_string(), *threshold);
        }
        filter
    }

    pub fn seed_from_states(&mut self, states: &[StateMessage]) {
        for state in states {
            self.last_states
                .insert(state.capability_id.clone(), state.value.clone());
        }
    }

    pub fn should_publish_and_remember(&mut self, state: &StateMessage) -> bool {
        let changed = has_meaningful_change(
            self.last_states.get(&state.capability_id),
            self.numeric_thresholds.get(&state.capability_id).copied(),
            &state.value,
        );

        if changed {
            self.last_states
                .insert(state.capability_id.clone(), state.value.clone());
        }

        changed
    }
}

fn has_meaningful_change(previous: Option<&Value>, threshold: Option<f64>, current: &Value) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    if let Some(threshold) = threshold {
        if let (Some(previous), Some(current)) = (previous.as_f64(), current.as_f64()) {
            return (current - previous).abs() >= threshold;
        }
    }

    previous != current
}

#[cfg(test)]
mod tests {
    use hs_device_contracts::StateMessage;
    use serde_json::json;

    use super::StateFilter;

    const POWER_THRESHOLDS: &[(&str, f64)] = &[
        ("power_w", 2.0),
        ("voltage_v", 0.1),
        ("current_a", 0.01),
        ("energy_total_kwh", 0.001),
    ];

    fn state(capability_id: &str, value: serde_json::Value) -> StateMessage {
        StateMessage {
            device_id: "device-1".to_string(),
            capability_id: capability_id.to_string(),
            value,
            observed_ms: 1,
        }
    }

    #[test]
    fn suppresses_small_energy_deltas_with_threshold() {
        let mut filter = StateFilter::with_numeric_thresholds(POWER_THRESHOLDS);

        assert!(filter.should_publish_and_remember(&state("energy_total_kwh", json!(145.1015453))));
        assert!(!filter.should_publish_and_remember(&state("energy_total_kwh", json!(145.1015458))));
        assert!(filter.should_publish_and_remember(&state("energy_total_kwh", json!(150.0))));
    }

    #[test]
    fn suppresses_exact_non_numeric_duplicates() {
        let mut filter = StateFilter::new();

        assert!(filter.should_publish_and_remember(&state("power", json!("ON"))));
        assert!(!filter.should_publish_and_remember(&state("power", json!("ON"))));
        assert!(filter.should_publish_and_remember(&state("power", json!("OFF"))));
    }

    #[test]
    fn suppresses_small_power_deltas_with_threshold() {
        let mut filter = StateFilter::with_numeric_thresholds(POWER_THRESHOLDS);

        assert!(filter.should_publish_and_remember(&state("power_w", json!(145.1))));
        assert!(!filter.should_publish_and_remember(&state("power_w", json!(145.9))));
        assert!(filter.should_publish_and_remember(&state("power_w", json!(150.0))));
    }
}