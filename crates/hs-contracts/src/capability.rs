use serde::{Deserialize, Serialize};

/// An opaque device class string passed through to the event bus adapter.
/// Use the constants in [`sensor_class`] and [`binary_sensor_class`] for
/// well-known values, or call `DeviceClass::new("your_class")` for anything
/// else — no recompilation of this crate is required.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceClass(pub String);

impl DeviceClass {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DeviceClass {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Well-known HA sensor device classes. Not exhaustive — use `DeviceClass::new`
/// for anything not listed here.
pub mod sensor_class {
    pub const TEMPERATURE: &str = "temperature";
    pub const HUMIDITY: &str = "humidity";
    pub const ILLUMINANCE: &str = "illuminance";
    pub const POWER: &str = "power";
    pub const ENERGY: &str = "energy";
    pub const VOLTAGE: &str = "voltage";
    pub const CURRENT: &str = "current";
    pub const BATTERY: &str = "battery";
    pub const SIGNAL_STRENGTH: &str = "signal_strength";
    pub const PRESSURE: &str = "pressure";
    pub const CO2: &str = "carbon_dioxide";
    pub const PM25: &str = "pm25";
    pub const PM10: &str = "pm10";
    pub const DISTANCE: &str = "distance";
    pub const SPEED: &str = "speed";
    pub const VOLUME: &str = "volume";
    pub const GAS: &str = "gas";
    pub const MOISTURE: &str = "moisture";
}

/// Well-known HA binary sensor device classes. Not exhaustive — use
/// `DeviceClass::new` for anything not listed here.
pub mod binary_sensor_class {
    pub const MOTION: &str = "motion";
    pub const OCCUPANCY: &str = "occupancy";
    pub const DOOR: &str = "door";
    pub const WINDOW: &str = "window";
    pub const SMOKE: &str = "smoke";
    pub const MOISTURE: &str = "moisture";
    pub const VIBRATION: &str = "vibration";
    pub const CONNECTIVITY: &str = "connectivity";
    pub const PLUG: &str = "plug";
    pub const LOCK: &str = "lock";
    pub const TAMPER: &str = "tamper";
    pub const GAS: &str = "gas";
    pub const COLD: &str = "cold";
    pub const HEAT: &str = "heat";
    pub const SOUND: &str = "sound";
    pub const LIGHT: &str = "light";
    pub const PROBLEM: &str = "problem";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightFeatures {
    pub brightness: bool,
    pub color_temp: bool,
    pub rgb: bool,
    pub effects: bool,
}

impl LightFeatures {
    pub fn on_off_only() -> Self {
        Self {
            brightness: false,
            color_temp: false,
            rgb: false,
            effects: false,
        }
    }

    pub fn dimmable() -> Self {
        Self {
            brightness: true,
            color_temp: false,
            rgb: false,
            effects: false,
        }
    }

    pub fn color_temp() -> Self {
        Self {
            brightness: true,
            color_temp: true,
            rgb: false,
            effects: false,
        }
    }

    pub fn full_color() -> Self {
        Self {
            brightness: true,
            color_temp: true,
            rgb: true,
            effects: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NumberConfig {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit_of_measurement: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapabilityKind {
    Sensor { device_class: Option<DeviceClass> },
    BinarySensor { device_class: Option<DeviceClass> },
    Switch,
    Button,
    Light { features: LightFeatures },
    Number { config: NumberConfig },
    Select { options: Vec<String> },
    Cover,
    Climate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub capability_id: String,
    pub kind: CapabilityKind,
    pub friendly_name: String,
    /// Unit displayed in UI. Redundant for some kinds (Number has it inside config)
    /// but kept here for simple sensor/binary_sensor use without extra nesting.
    pub unit_of_measurement: Option<String>,
}
