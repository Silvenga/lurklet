use crate::error::ApiError;
use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use lurklet_reddit::{
    RedditBackend, RedditClient, Subreddit as RedditSubreddit,
    SubredditDetails as RedditSubredditDetails, SubredditStyles as RedditSubredditStyles,
    UnavailableSubreddit as RedditUnavailableSubreddit,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

const CACHE_POLICY: &str = "public, max-age=300";

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Path)]
struct GetSubredditInput {
    /// Community name, without the r/ prefix.
    subreddit: String,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Subreddit {
    Available(Box<SubredditDetails>),
    Unavailable(UnavailableSubreddit),
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SubredditDetails {
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
    description_markdown: Option<String>,
    #[schema(required)]
    subscriber_count: Option<u64>,
    /// Weekly activity, not a count of currently online users.
    #[schema(required)]
    weekly_active_user_count: Option<u64>,
    #[schema(required)]
    weekly_contribution_count: Option<u64>,
    #[schema(required)]
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
    quarantine_message_markdown: Option<String>,
    #[schema(required)]
    interstitial_warning_markdown: Option<String>,
}

#[derive(Serialize, ToSchema)]
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

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UnavailableSubreddit {
    #[schema(required)]
    id: Option<String>,
    #[schema(required)]
    forbidden_reason: Option<String>,
    #[schema(required)]
    ban_title: Option<String>,
    #[schema(required)]
    ban_message_markdown: Option<String>,
    #[schema(required)]
    quarantine_message_markdown: Option<String>,
    #[schema(required)]
    interstitial_warning_markdown: Option<String>,
}

impl From<RedditSubreddit> for Subreddit {
    fn from(subreddit: RedditSubreddit) -> Self {
        match subreddit {
            RedditSubreddit::Available(details) => Self::Available(Box::new((*details).into())),
            RedditSubreddit::Unavailable(details) => Self::Unavailable(details.into()),
        }
    }
}

impl From<RedditSubredditDetails> for SubredditDetails {
    fn from(details: RedditSubredditDetails) -> Self {
        let RedditSubredditDetails {
            id,
            name,
            title,
            created_at,
            short_description,
            description_markdown,
            subscriber_count,
            weekly_active_user_count,
            weekly_contribution_count,
            styles,
            community_type,
            is_nsfw,
            is_posting_restricted,
            is_quarantined,
            quarantine_message_markdown,
            interstitial_warning_markdown,
        } = details;
        Self {
            id: id.as_str().to_owned(),
            name,
            title,
            created_at,
            short_description,
            description_markdown,
            subscriber_count,
            weekly_active_user_count,
            weekly_contribution_count,
            styles: styles.map(Into::into),
            community_type,
            is_nsfw,
            is_posting_restricted,
            is_quarantined,
            quarantine_message_markdown,
            interstitial_warning_markdown,
        }
    }
}

impl From<RedditSubredditStyles> for SubredditStyles {
    fn from(styles: RedditSubredditStyles) -> Self {
        let RedditSubredditStyles {
            icon_url,
            legacy_icon_url,
            banner_url,
            mobile_banner_url,
            legacy_banner_url,
            primary_color,
            legacy_primary_color,
        } = styles;
        Self {
            icon_url,
            legacy_icon_url,
            banner_url,
            mobile_banner_url,
            legacy_banner_url,
            primary_color,
            legacy_primary_color,
        }
    }
}

impl From<RedditUnavailableSubreddit> for UnavailableSubreddit {
    fn from(details: RedditUnavailableSubreddit) -> Self {
        let RedditUnavailableSubreddit {
            id,
            forbidden_reason,
            ban_title,
            ban_message_markdown,
            quarantine_message_markdown,
            interstitial_warning_markdown,
        } = details;
        Self {
            id: id.map(|id| id.as_str().to_owned()),
            forbidden_reason,
            ban_title,
            ban_message_markdown,
            quarantine_message_markdown,
            interstitial_warning_markdown,
        }
    }
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
    Ok((
        [(header::CACHE_CONTROL, CACHE_POLICY)],
        Json(Subreddit::from(subreddit)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResponseCache;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use axum::routing::get;
    use lurklet_reddit::{
        AnonymousSession, CommentForest, CommentSort, CommentThread, Error, MediaFile, Post,
        RedditId, SubredditPage, SubredditSort,
    };
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    struct FakeBackend {
        result: Result<RedditSubreddit, Error>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl RedditBackend for FakeBackend {
        async fn get_subreddit(&self, name: &str) -> Result<RedditSubreddit, Error> {
            self.calls.lock().unwrap().push(name.into());
            self.result.clone()
        }

        async fn get_media(&self, _: &str) -> Result<MediaFile, Error> {
            panic!("the metadata endpoint must not fetch assets")
        }

        async fn get_post(&self, _: &RedditId) -> Result<Post, Error> {
            panic!("the metadata endpoint must not fetch post details")
        }

        async fn get_post_comments(
            &self,
            _: &RedditId,
            _: CommentSort,
            _: Option<&str>,
        ) -> Result<CommentForest, Error> {
            panic!("the metadata endpoint must not fetch comments")
        }

        async fn get_comment_thread(
            &self,
            _: &RedditId,
            _: CommentSort,
        ) -> Result<CommentThread, Error> {
            panic!("the metadata endpoint must not fetch comment threads")
        }

        async fn list_subreddit_posts(
            &self,
            _: &str,
            _: SubredditSort,
            _: Option<&str>,
        ) -> Result<SubredditPage, Error> {
            panic!("the metadata endpoint must not fetch a feed")
        }

        async fn bootstrap(&self) -> Result<AnonymousSession, Error> {
            panic!("the endpoint must leave bootstrap to the client backend")
        }
    }

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
        RedditSubreddit::Unavailable(RedditUnavailableSubreddit {
            id: None,
            forbidden_reason: Some("FUTURE_REASON".into()),
            ban_title: Some("Community unavailable".into()),
            ban_message_markdown: Some("A **reported** explanation".into()),
            quarantine_message_markdown: None,
            interstitial_warning_markdown: Some("Untrusted <em>warning</em>".into()),
        })
    }

    fn app(result: Result<RedditSubreddit, Error>, calls: Arc<Mutex<Vec<String>>>) -> Router {
        router()
            .split_for_parts()
            .0
            .with_state(RedditClient::from_backend(FakeBackend { result, calls }))
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
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
        assert_eq!(*calls.lock().unwrap(), ["Rust"]);
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({
                "kind": "available", "id": "community", "name": "Rust", "title": "Rust programming",
                "createdAt": "1970-01-01T00:00:00Z", "shortDescription": "Systems programming",
                "descriptionMarkdown": "**Untrusted** <em>Markdown</em>",
                "subscriberCount": 12345, "weeklyActiveUserCount": 678, "weeklyContributionCount": 90,
                "styles": {
                    "iconUrl": "https://example.test/icon.png", "legacyIconUrl": null,
                    "bannerUrl": "https://example.test/banner.png", "mobileBannerUrl": null,
                    "legacyBannerUrl": "https://example.test/legacy-banner.png",
                    "primaryColor": "#123456", "legacyPrimaryColor": null
                },
                "communityType": "FUTURE_COMMUNITY_TYPE", "isNsfw": false,
                "isPostingRestricted": true, "isQuarantined": null,
                "quarantineMessageMarkdown": "Quarantine **notice**", "interstitialWarningMarkdown": null
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
            "public, max-age=300"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), 4096).await.unwrap())
                .unwrap(),
            json!({
                "kind": "unavailable", "id": null, "forbiddenReason": "FUTURE_REASON",
                "banTitle": "Community unavailable", "banMessageMarkdown": "A **reported** explanation",
                "quarantineMessageMarkdown": null, "interstitialWarningMarkdown": "Untrusted <em>warning</em>"
            })
        );
        assert_eq!(*calls.lock().unwrap(), ["example"]);
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
        let unavailable = RedditSubreddit::Unavailable(RedditUnavailableSubreddit {
            id: Some("community".into()),
            forbidden_reason: None,
            ban_title: None,
            ban_message_markdown: None,
            quarantine_message_markdown: None,
            interstitial_warning_markdown: None,
        });

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
        for error in [Error::Unavailable, Error::NotFound] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = app(Err(error), calls.clone());

            let response = app.oneshot(request("missing")).await.unwrap();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=10"
            );
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/problem+json"
            );
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &to_bytes(response.into_body(), 4096).await.unwrap()
                )
                .unwrap(),
                json!({"type": "about:blank", "title": "Not Found", "status": 404})
            );
            assert_eq!(*calls.lock().unwrap(), ["missing"]);
        }
    }

    #[tokio::test]
    async fn when_metadata_is_cached_then_get_and_head_should_reuse_separate_community_entries() {
        for result in [available(), unavailable()] {
            let directory = assert_fs::TempDir::new().unwrap();
            let cache = ResponseCache::new(directory.path()).await.unwrap();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let app = cache.wrap(app(Ok(result), calls.clone()));

            for (name, outcome) in [("Rust", "miss"), ("Rust", "hit"), ("Other", "miss")] {
                let response = app.clone().oneshot(request(name)).await.unwrap();

                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(response.headers()["x-lurklet-cache"], outcome);
                assert_eq!(
                    response.headers()[header::CACHE_CONTROL],
                    "public, max-age=300"
                );
                assert!(response.headers().contains_key(header::AGE));
            }
            let mut head = request("Rust");
            *head.method_mut() = Method::HEAD;

            let response = app.oneshot(head).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["x-lurklet-cache"], "hit");
            assert!(
                to_bytes(response.into_body(), 16_384)
                    .await
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(*calls.lock().unwrap(), ["Rust", "Other"]);
        }
    }

    #[tokio::test]
    async fn when_a_metadata_refresh_fails_then_the_stored_representation_and_policy_should_be_preserved()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let seed = cache.wrap(Router::new().route(
            "/subreddits/Rust",
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=0")],
                    Json(Subreddit::from(available())),
                )
            }),
        ));
        let response = seed.oneshot(request("Rust")).await.unwrap();
        let expected = to_bytes(response.into_body(), 16_384).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = cache.wrap(app(Err(Error::Unavailable), calls.clone()));

        let response = app.oneshot(request("Rust")).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-lurklet-cache"], "stale");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=0"
        );
        assert!(response.headers().contains_key(header::AGE));
        assert_eq!(
            to_bytes(response.into_body(), 16_384).await.unwrap(),
            expected
        );
        assert_eq!(*calls.lock().unwrap(), ["Rust"]);
    }
}
