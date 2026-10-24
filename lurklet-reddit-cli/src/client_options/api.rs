use clap::Args;
use lurklet_reddit::RedditClientBuilder;

#[derive(Args, Default)]
pub struct ApiOptions {
    /// Trusted HTTPS destination for Basic-authenticated bootstrap requests.
    #[arg(long, global = true)]
    auth_endpoint: Option<String>,
    /// Trusted HTTPS destination for future GraphQL operations.
    #[arg(long, global = true)]
    graphql_endpoint: Option<String>,
    #[arg(long, global = true)]
    client_id: Option<String>,
    /// Override the complete scope list. Repeat this flag for each scope.
    #[arg(long = "scope", global = true)]
    scopes: Vec<String>,
}

impl ApiOptions {
    pub fn apply(self, mut builder: RedditClientBuilder) -> RedditClientBuilder {
        if let Some(value) = self.auth_endpoint {
            builder = builder.with_auth_endpoint(value);
        }
        if let Some(value) = self.graphql_endpoint {
            builder = builder.with_graphql_endpoint(value);
        }
        if let Some(value) = self.client_id {
            builder = builder.with_client_id(value);
        }
        if !self.scopes.is_empty() {
            builder = builder.with_scopes(self.scopes);
        }
        builder
    }
}
