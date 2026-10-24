use clap::Args;
use lurklet_reddit::{AppProfile, RedditClientBuilder};

#[derive(Args, Default)]
pub struct AppOptions {
    #[arg(long, global = true)]
    app_version: Option<String>,
    #[arg(long, global = true)]
    app_build: Option<String>,
    #[arg(long, global = true)]
    user_agent: Option<String>,
}

impl AppOptions {
    pub fn apply(self, mut builder: RedditClientBuilder) -> RedditClientBuilder {
        let mut app = AppProfile::default();
        if let Some(value) = self.app_version {
            app.version = value;
        }
        if let Some(value) = self.app_build {
            app.build = value;
        }
        builder = builder.with_app_profile(app);
        if let Some(value) = self.user_agent {
            builder = builder.with_user_agent(value);
        }
        builder
    }
}
