use crate::error::ApiError;
use crate::routes::Markdown;
use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use lurklet_reddit::{
    RedditBackend, RedditClient, Subreddit as RedditSubreddit,
    SubredditDetails as RedditSubredditDetails, SubredditStyles as RedditSubredditStyles,
    UnavailableSubreddit as RedditUnavailableSubreddit,
};
use o2o::o2o;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Path)]
struct GetSubredditInput {
    /// Community name, without the r/ prefix.
    subreddit: String,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditSubreddit)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Subreddit {
    Available(#[from(Box::new((*~).into()))] Box<SubredditDetails>),
    Unavailable(#[from(Box::new((*~).into()))] Box<UnavailableSubreddit>),
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditSubredditDetails)]
#[serde(rename_all = "camelCase")]
struct SubredditDetails {
    #[from(~.as_str().to_owned())]
    id: String,
    name: String,
    #[schema(required)]
    title: Option<String>,
    #[schema(required)]
    created_at: Option<DateTime<Utc>>,
    #[schema(required)]
    short_description: Option<String>,
    /// Untrusted long-form Markdown, not structured sidebar widgets or rules.
    #[schema(required)]
    #[from(~.map(Into::into))]
    description_markdown: Option<Markdown>,
    #[schema(required)]
    subscriber_count: Option<u64>,
    /// Weekly activity, not a count of currently online users.
    #[schema(required)]
    weekly_active_user_count: Option<u64>,
    #[schema(required)]
    weekly_contribution_count: Option<u64>,
    #[schema(required)]
    #[from(~.map(Into::into))]
    styles: Option<SubredditStyles>,
    #[schema(required)]
    community_type: Option<String>,
    #[schema(required)]
    is_nsfw: Option<bool>,
    #[schema(required)]
    is_posting_restricted: Option<bool>,
    #[schema(required)]
    is_quarantined: Option<bool>,
    #[schema(required)]
    #[from(~.map(Into::into))]
    quarantine_message_markdown: Option<Markdown>,
    #[schema(required)]
    #[from(~.map(Into::into))]
    interstitial_warning_markdown: Option<Markdown>,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditSubredditStyles)]
#[serde(rename_all = "camelCase")]
struct SubredditStyles {
    #[schema(required)]
    icon_url: Option<String>,
    #[schema(required)]
    legacy_icon_url: Option<String>,
    #[schema(required)]
    banner_url: Option<String>,
    #[schema(required)]
    mobile_banner_url: Option<String>,
    #[schema(required)]
    legacy_banner_url: Option<String>,
    #[schema(required)]
    primary_color: Option<String>,
    #[schema(required)]
    legacy_primary_color: Option<String>,
}

#[derive(Serialize, ToSchema, o2o)]
#[from_owned(RedditUnavailableSubreddit)]
#[serde(rename_all = "camelCase")]
struct UnavailableSubreddit {
    #[schema(required)]
    #[from(~.map(|id| id.as_str().to_owned()))]
    id: Option<String>,
    #[schema(required)]
    forbidden_reason: Option<String>,
    #[schema(required)]
    ban_title: Option<String>,
    #[schema(required)]
    #[from(~.map(Into::into))]
    ban_message_markdown: Option<Markdown>,
    #[schema(required)]
    #[from(~.map(Into::into))]
    quarantine_message_markdown: Option<Markdown>,
    #[schema(required)]
    #[from(~.map(Into::into))]
    interstitial_warning_markdown: Option<Markdown>,
}

/// Builds the subreddit metadata endpoint and its API-owned contract.
pub fn router<B: RedditBackend + 'static>() -> OpenApiRouter<RedditClient<B>> {
    OpenApiRouter::new().routes(routes!(get_subreddit))
}

#[utoipa::path(
    get,
    path = "/subreddits/{subreddit}",
    operation_id = "get_subreddit",
    summary = "Read community metadata without fetching its feed or assets",
    params(GetSubredditInput),
    responses(
        ApiError,
        (status = 200, description = "Community metadata or an unavailable-community explanation", body = Subreddit)
    )
)]
async fn get_subreddit<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    input: Result<Path<GetSubredditInput>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Path(input) = input.map_err(|_| ApiError::BadRequest)?;
    let subreddit = reddit.get_subreddit(&input.subreddit).await?;
    Ok(Json(Subreddit::from(subreddit)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiCacheOptions;
    use crate::routes::testing::{Call, FakeBackend};
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use lurklet_reddit::Error;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    fn available() -> RedditSubreddit {
        RedditSubreddit::Available(Box::new(RedditSubredditDetails {
            id: "community".into(),
            name: "Rust".into(),
            title: Some("Rust programming".into()),
            created_at: Some(DateTime::from_timestamp(0, 0).unwrap()),
            short_description: Some("Systems programming".into()),
            description_markdown: Some("**Untrusted** <em>Markdown</em>".into()),
            subscriber_count: Some(12345),
            weekly_active_user_count: Some(678),
            weekly_contribution_count: Some(90),
            styles: Some(RedditSubredditStyles {
                icon_url: Some("https://example.test/icon.png".into()),
                legacy_icon_url: None,
                banner_url: Some("https://example.test/banner.png".into()),
                mobile_banner_url: None,
                legacy_banner_url: Some("https://example.test/legacy-banner.png".into()),
                primary_color: Some("#123456".into()),
                legacy_primary_color: None,
            }),
            community_type: Some("FUTURE_COMMUNITY_TYPE".into()),
            is_nsfw: Some(false),
            is_posting_restricted: Some(true),
            is_quarantined: None,
            quarantine_message_markdown: Some("Quarantine **notice**".into()),
            interstitial_warning_markdown: None,
        }))
    }

    fn unavailable() -> RedditSubreddit {
        RedditSubreddit::Unavailable(Box::new(RedditUnavailableSubreddit {
            id: None,
            forbidden_reason: Some("FUTURE_REASON".into()),
            ban_title: Some("Community unavailable".into()),
            ban_message_markdown: Some("A **reported** explanation".into()),
            quarantine_message_markdown: None,
            interstitial_warning_markdown: Some("Untrusted <em>warning</em>".into()),
        }))
    }

    fn app(result: Result<RedditSubreddit, Error>, calls: Arc<Mutex<Vec<Call>>>) -> Router {
        let routes = router()
            .split_for_parts()
            .0
            .with_state(RedditClient::from_backend(FakeBackend {
                subreddit: Some(result),
                calls,
                ..FakeBackend::default()
            }));
        ApiCacheOptions::default().wrap(routes)
    }

    fn request(name: &str) -> Request<Body> {
        Request::builder()
            .uri(format!("/subreddits/{name}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn when_metadata_is_requested_then_the_name_should_pass_unchanged_and_the_response_should_preserve_display_fields()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(available()), calls.clone());

        let response = app.oneshot(request("Rust")).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 16_384).await.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=3600");
        assert_eq!(*calls.lock().unwrap(), [Call::Subreddit("Rust".into())]);
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({
                "kind": "available", "id": "community", "name": "Rust", "title": "Rust programming",
                "createdAt": "1970-01-01T00:00:00Z", "shortDescription": "Systems programming",
                "descriptionMarkdown": {"content": "**Untrusted** <em>Markdown</em>", "media": []},
                "subscriberCount": 12345, "weeklyActiveUserCount": 678, "weeklyContributionCount": 90,
                "styles": {
                    "iconUrl": "https://example.test/icon.png", "legacyIconUrl": null,
                    "bannerUrl": "https://example.test/banner.png", "mobileBannerUrl": null,
                    "legacyBannerUrl": "https://example.test/legacy-banner.png",
                    "primaryColor": "#123456", "legacyPrimaryColor": null
                },
                "communityType": "FUTURE_COMMUNITY_TYPE", "isNsfw": false,
                "isPostingRestricted": true, "isQuarantined": null,
                "quarantineMessageMarkdown": {"content": "Quarantine **notice**", "media": []}, "interstitialWarningMarkdown": null
            })
        );
    }

    #[tokio::test]
    async fn when_unavailability_has_an_explanation_then_the_endpoint_should_return_a_cacheable_success()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(unavailable()), calls.clone());

        let response = app.oneshot(request("example")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=3600"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), 4096).await.unwrap())
                .unwrap(),
            json!({
                "kind": "unavailable", "id": null, "forbiddenReason": "FUTURE_REASON",
                "banTitle": "Community unavailable", "banMessageMarkdown": {"content": "A **reported** explanation", "media": []},
                "quarantineMessageMarkdown": null, "interstitialWarningMarkdown": {"content": "Untrusted <em>warning</em>", "media": []}
            })
        );
        assert_eq!(*calls.lock().unwrap(), [Call::Subreddit("example".into())]);
    }

    #[test]
    fn when_optional_metadata_is_absent_then_conversion_should_preserve_nulls_without_inference() {
        let available = RedditSubreddit::Available(Box::new(RedditSubredditDetails {
            id: "community".into(),
            name: "example".into(),
            title: None,
            created_at: None,
            short_description: None,
            description_markdown: None,
            subscriber_count: None,
            weekly_active_user_count: None,
            weekly_contribution_count: None,
            styles: None,
            community_type: None,
            is_nsfw: None,
            is_posting_restricted: None,
            is_quarantined: None,
            quarantine_message_markdown: None,
            interstitial_warning_markdown: None,
        }));
        let unavailable = RedditSubreddit::Unavailable(Box::new(RedditUnavailableSubreddit {
            id: Some("community".into()),
            forbidden_reason: None,
            ban_title: None,
            ban_message_markdown: None,
            quarantine_message_markdown: None,
            interstitial_warning_markdown: None,
        }));

        let available = serde_json::to_value(Subreddit::from(available)).unwrap();
        let unavailable = serde_json::to_value(Subreddit::from(unavailable)).unwrap();

        assert_eq!(
            available,
            json!({
                "kind": "available", "id": "community", "name": "example", "title": null,
                "createdAt": null, "shortDescription": null, "descriptionMarkdown": null,
                "subscriberCount": null, "weeklyActiveUserCount": null, "weeklyContributionCount": null,
                "styles": null, "communityType": null, "isNsfw": null, "isPostingRestricted": null,
                "isQuarantined": null, "quarantineMessageMarkdown": null, "interstitialWarningMarkdown": null
            })
        );
        assert_eq!(
            unavailable,
            json!({
                "kind": "unavailable", "id": "community", "forbiddenReason": null, "banTitle": null,
                "banMessageMarkdown": null, "quarantineMessageMarkdown": null, "interstitialWarningMarkdown": null
            })
        );
    }

    #[tokio::test]
    async fn when_the_path_cannot_be_decoded_then_the_client_should_not_be_called() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Ok(available()), calls.clone());

        let response = app.oneshot(request("%FF")).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=10"
        );
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), 4096).await.unwrap())
                .unwrap(),
            json!({"type": "about:blank", "title": "Bad Request", "status": 400})
        );
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_the_resource_is_unavailable_without_metadata_then_the_endpoint_should_return_not_found()
     {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = app(Err(Error::Unavailable), calls.clone());

        let response = app.oneshot(request("missing")).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
        assert_eq!(*calls.lock().unwrap(), [Call::Subreddit("missing".into())]);
    }
}
