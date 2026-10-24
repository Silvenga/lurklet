use serde::{Deserialize, Serialize};

/// Stable application version metadata used to derive the default user-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProfile {
    pub version: String,
    pub build: String,
}

impl Default for AppProfile {
    fn default() -> Self {
        Self {
            version: "2026.42.0".into(),
            build: "2642021".into(),
        }
    }
}

/// A consistent device profile; values are not randomized between requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub android_release: String,
    pub manufacturer: String,
    pub model: String,
    pub locale: String,
    pub display_density: f64,
    pub display_width_px: u32,
    pub display_height_px: u32,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            android_release: "14".into(),
            manufacturer: "Google".into(),
            model: "Pixel 7".into(),
            locale: "en-US".into(),
            display_density: 2.75,
            display_width_px: 1080,
            display_height_px: 2400,
        }
    }
}
