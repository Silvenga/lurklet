use clap::Args;
use lurklet_reddit::{DeviceProfile, RedditClientBuilder};

#[derive(Args, Default)]
pub struct DeviceOptions {
    #[arg(long, global = true)]
    android_release: Option<String>,
    #[arg(long, global = true)]
    manufacturer: Option<String>,
    #[arg(long, global = true)]
    model: Option<String>,
    #[arg(long, global = true)]
    locale: Option<String>,
    #[arg(long, global = true, allow_hyphen_values = true)]
    display_density: Option<f64>,
    #[arg(long, global = true)]
    display_width_px: Option<u32>,
    #[arg(long, global = true)]
    display_height_px: Option<u32>,
    #[arg(long, global = true, num_args = 0..=1, default_missing_value = "true")]
    height_header: Option<bool>,
}

impl DeviceOptions {
    pub fn apply(self, mut builder: RedditClientBuilder) -> RedditClientBuilder {
        let mut device = DeviceProfile::default();
        if let Some(value) = self.android_release {
            device.android_release = value;
        }
        if let Some(value) = self.manufacturer {
            device.manufacturer = value;
        }
        if let Some(value) = self.model {
            device.model = value;
        }
        if let Some(value) = self.locale {
            device.locale = value;
        }
        if let Some(value) = self.display_density {
            device.display_density = value;
        }
        if let Some(value) = self.display_width_px {
            device.display_width_px = value;
        }
        if let Some(value) = self.display_height_px {
            device.display_height_px = value;
        }
        builder = builder.with_device_profile(device);
        if let Some(value) = self.height_header {
            builder = builder.with_height_header(value);
        }
        builder
    }
}
