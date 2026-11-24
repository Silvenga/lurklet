use crate::error::ApiError;
use crate::middleware::ClientIpKey;
use crate::routes::Markdown;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{ConnectInfo, FromRequestParts, OriginalUri, Path, Query, State};
use axum::http::header::HOST;
use axum::http::request::Parts;
use axum::http::uri::Authority;
use axum::routing::get;
use axum::{Json, Router};
use lurklet_reddit::{
    CommentItem, CommentSort, Error, PostAttachment, RedditBackend, RedditClient, RedditId,
    Subreddit, SubredditSort,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fmt::Display;
use std::net::SocketAddr;

pub const LISTING: &str = "/r/{subreddit}";
pub const POST: &str = "/r/{subreddit}/comments/{postId}";
pub const REPLIES: &str = "/r/{subreddit}/comments/{postId}/replies/{cursor}";
pub const THREAD: &str = "/r/{subreddit}/comments/{postId}/threads/{commentId}";

/// Builds the agent views independently of the SPA's OpenAPI contract.
pub fn router<B: RedditBackend + 'static>() -> Router<RedditClient<B>> {
    Router::new()
        .route(LISTING, get(listing::<B>))
        .route(POST, get(post::<B>))
        .route(REPLIES, get(replies::<B>))
        .route(THREAD, get(thread::<B>))
}

struct Origin(String);

impl<S: Sync> FromRequestParts<S> for Origin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        let uri = parts
            .extensions
            .get::<OriginalUri>()
            .map_or(&parts.uri, |original| &original.0);
        let authority = match parts.headers.get(HOST) {
            Some(host) => Authority::try_from(host.as_bytes()).map_err(|_| ApiError::BadRequest)?,
            None => uri.authority().cloned().ok_or(ApiError::BadRequest)?,
        };
        if !authority
            .as_str()
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-_:[]".contains(&c))
            || authority.host().is_empty()
        {
            return Err(ApiError::BadRequest);
        }
        let trusted = parts
            .extensions
            .get::<ClientIpKey>()
            .zip(parts.extensions.get::<ConnectInfo<SocketAddr>>())
            .is_some_and(|(policy, peer)| policy.trusts_forwarded_scheme(peer.0.ip()));
        let scheme = if trusted
            && parts
                .headers
                .get("x-forwarded-proto")
                .is_some_and(|v| v == "https")
        {
            "https"
        } else {
            "http"
        };
        Ok(Self(format!("{scheme}://{authority}")))
    }
}

fn segment(value: &str) -> impl Display + '_ {
    const ENCODE: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    utf8_percent_encode(value, ENCODE)
}

impl Origin {
    fn listing(&self, subreddit: &str) -> String {
        format!("{}/agents/r/{}", self.0, segment(subreddit))
    }

    fn post(&self, subreddit: &str, id: &RedditId) -> String {
        format!(
            "{}/comments/{}",
            self.listing(subreddit),
            segment(id.as_str())
        )
    }
}

fn comments(items: Vec<CommentItem>, post_url: &str) -> Vec<Value> {
    items.into_iter().map(|item| match item {
        CommentItem::Comment { comment, replies } => json!({
            "id": comment.id, "state": comment.state,
            "author": comment.author.and_then(|a| a.name), "createdAt": comment.created_at,
            "score": comment.score, "markdown": comment.markdown.map(Markdown::from),
            "replies": comments(replies, post_url),
        }),
        CommentItem::More { cursor, .. } => json!({"moreRepliesUrl": cursor.map(|s| format!("{post_url}/replies/{}", segment(&s)))}),
        CommentItem::ContinueThread { comment_id } => json!({"threadUrl": comment_id.map(|id| format!("{post_url}/threads/{}", segment(id.as_str())))}),
    }).collect()
}

#[derive(Deserialize)]
struct ListingQuery {
    after: Option<String>,
}

fn continuation_error(error: Error) -> ApiError {
    match error {
        Error::InvalidRequest {
            field: "cursor" | "after",
        } => ApiError::BadRequest,
        other => other.into(),
    }
}

async fn listing<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    origin: Origin,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<ListingQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let Path(subreddit) = path.map_err(|_| ApiError::BadRequest)?;
    let Query(query) = query.map_err(|_| ApiError::BadRequest)?;
    let page = reddit
        .list_subreddit_posts(&subreddit, SubredditSort::Hot, query.after.as_deref())
        .await
        .map_err(continuation_error)?;
    let posts: Vec<_> = page.value.posts.into_iter().map(|p| json!({
        "id": p.id, "title": p.title, "author": p.author_name, "createdAt": p.created_at, "score": p.score,
        "commentCount": p.comment_count, "postUrl": origin.post(&subreddit, &p.id),
    })).collect();
    let next = page
        .value
        .next_cursor
        .map(|cursor| format!("{}?after={}", origin.listing(&subreddit), segment(&cursor)));
    Ok(Json(
        json!({"fetchedAt": page.fetched_at, "posts": posts, "nextPageUrl": next}),
    ))
}

async fn post<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    origin: Origin,
    path: Result<Path<(String, RedditId)>, PathRejection>,
) -> Result<Json<Value>, ApiError> {
    let Path((name, id)) = path.map_err(|_| ApiError::BadRequest)?;
    let post = reddit.get_post(&id).await?;
    let p = post.value;
    if p.community
        .as_ref()
        .and_then(|c| c.name.as_deref())
        .is_some_and(|c| !c.eq_ignore_ascii_case(&name))
    {
        return Err(ApiError::NotFound);
    }
    let (subreddit, forest) = tokio::try_join!(
        reddit.get_subreddit(&name),
        reddit.get_post_comments(&id, CommentSort::Best, None)
    )?;
    let Subreddit::Available(s) = subreddit.value else {
        return Err(ApiError::NotFound);
    };
    let mut links = Vec::new();
    let mut attachments = Vec::new();
    for attachment in p.attachments {
        match attachment {
            PostAttachment::Link { url } => links.push(url),
            PostAttachment::Image {
                url,
                alt_text,
                caption,
            } => attachments
                .push(json!({"kind":"image", "url":url, "altText":alt_text, "caption":caption})),
            PostAttachment::Video {
                sources,
                poster_url,
            } => {
                attachments.push(json!({"kind":"video", "sources":sources, "posterUrl":poster_url}))
            }
        }
    }
    Ok(Json(json!({
        "fetchedAt": {"subreddit": subreddit.fetched_at, "post": post.fetched_at, "comments": forest.fetched_at},
        "subreddit": {"name": s.name, "description": s.short_description, "subscribers": s.subscriber_count,
            "createdAt": s.created_at, "weeklyActiveUsers": s.weekly_active_user_count, "weeklyContributions": s.weekly_contribution_count},
        "post": {"id": p.id, "author": p.author.and_then(|a| a.name), "title": p.title, "createdAt": p.created_at,
            "score": p.score, "markdown": p.markdown.map(Markdown::from), "attachments": attachments, "links": links},
        "comments": comments(forest.value.items, &origin.post(&name, &id)),
    })))
}

async fn replies<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    origin: Origin,
    path: Result<Path<(String, RedditId, String)>, PathRejection>,
) -> Result<Json<Value>, ApiError> {
    let Path((name, id, cursor)) = path.map_err(|_| ApiError::BadRequest)?;
    let forest = reddit
        .get_post_comments(&id, CommentSort::Best, Some(&cursor))
        .await
        .map_err(continuation_error)?;
    Ok(Json(
        json!({"fetchedAt": forest.fetched_at, "comments": comments(forest.value.items, &origin.post(&name, &id))}),
    ))
}

async fn thread<B: RedditBackend>(
    State(reddit): State<RedditClient<B>>,
    origin: Origin,
    path: Result<Path<(String, RedditId, RedditId)>, PathRejection>,
) -> Result<Json<Value>, ApiError> {
    let Path((name, id, comment)) = path.map_err(|_| ApiError::BadRequest)?;
    let thread = reddit
        .get_comment_thread(&comment, CommentSort::Best)
        .await?;
    if thread
        .value
        .post_id
        .as_ref()
        .is_some_and(|post| post != &id)
    {
        return Err(ApiError::NotFound);
    }
    Ok(Json(
        json!({"fetchedAt": thread.fetched_at, "comments": comments(thread.value.forest.items, &origin.post(&name, &id))}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::testing::{Call, FakeBackend};
    use crate::{ApiCacheOptions, RateLimits, agents_service};
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, Uri};
    use chrono::{DateTime, Utc};
    use lurklet_reddit::{CommentForest, CommentThread, NamedReference, Post, SubredditDetails};
    use tower::ServiceExt;

    #[tokio::test]
    async fn when_agent_routes_are_disabled_then_requests_should_be_forbidden_without_upstream_reads()
     {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let app = agents_service(
            RedditClient::from_backend(backend),
            &RateLimits::default(),
            &ApiCacheOptions::default(),
            false,
        );

        for path in ["/agents", "/agents/", "/agents/r/rust"] {
            let request = Request::builder().uri(path).body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_following_comment_links_then_only_the_requested_branch_should_be_fetched() {
        let backend = FakeBackend {
            post: Some(Ok(Post {
                id: "post".into(),
                title: "Title".into(),
                permalink: None,
                flair: None,
                author: None,
                community: Some(NamedReference {
                    id: None,
                    name: Some("RUST".into()),
                }),
                created_at: Default::default(),
                score: None,
                comment_count: None,
                is_nsfw: false,
                is_spoiler: false,
                markdown: None,
                attachments: vec![],
                crosspost_id: None,
            })),
            subreddit: Some(Ok(Subreddit::Available(Box::new(SubredditDetails {
                id: "sub".into(),
                name: "Rust".into(),
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
            })))),
            comments: Some(Ok(CommentForest {
                applied_sort: None,
                items: vec![
                    CommentItem::More {
                        cursor: Some("a+/=%".into()),
                        count_hint: None,
                    },
                    CommentItem::ContinueThread {
                        comment_id: Some("anchor".into()),
                    },
                ],
            })),
            thread: Some(Ok(CommentThread {
                post_id: None,
                forest: CommentForest {
                    applied_sort: None,
                    items: vec![],
                },
            })),
            ..FakeBackend::default()
        };
        let calls = backend.calls.clone();
        let app = agents_service(
            RedditClient::from_backend(backend),
            &RateLimits::default(),
            &ApiCacheOptions::default(),
            true,
        );
        let read = |url: Uri| {
            let app = app.clone();
            async move {
                let request = Request::builder()
                    .uri(url)
                    .header(HOST, "test.example")
                    .extension(ConnectInfo("127.0.0.1:80".parse::<SocketAddr>().unwrap()))
                    .body(Body::empty())
                    .unwrap();
                let response = app.oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                serde_json::from_slice::<Value>(
                    &to_bytes(response.into_body(), 8192).await.unwrap(),
                )
                .unwrap()
            }
        };

        let combined = read("/agents/r/rust/comments/post".parse().unwrap()).await;
        for source in ["subreddit", "post", "comments"] {
            assert!(
                combined["fetchedAt"][source]
                    .as_str()
                    .unwrap()
                    .parse::<DateTime<Utc>>()
                    .is_ok()
            );
        }
        for (index, key) in [(0, "moreRepliesUrl"), (1, "threadUrl")] {
            let url: Uri = combined["comments"][index][key]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(url.authority().unwrap(), "test.example");
            let branch = read(url).await;
            assert!(
                branch["fetchedAt"]
                    .as_str()
                    .unwrap()
                    .parse::<DateTime<Utc>>()
                    .is_ok()
            );
            assert!(branch["comments"].is_array());
        }

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 5);
        assert!(calls[..3].contains(&Call::Post("post".into())));
        assert!(calls[..3].contains(&Call::Subreddit("rust".into())));
        assert!(calls[..3].contains(&Call::Comments {
            id: "post".into(),
            sort: CommentSort::Best,
            cursor: None
        }));
        assert_eq!(
            calls[3..],
            [
                Call::Comments {
                    id: "post".into(),
                    sort: CommentSort::Best,
                    cursor: Some("a+/=%".into())
                },
                Call::Thread {
                    id: "anchor".into(),
                    sort: CommentSort::Best
                }
            ]
        );
    }

    #[tokio::test]
    async fn when_a_thread_belongs_to_another_post_then_its_single_read_should_return_not_found() {
        let backend = FakeBackend {
            thread: Some(Ok(CommentThread {
                post_id: Some("other".into()),
                forest: CommentForest {
                    applied_sort: None,
                    items: vec![],
                },
            })),
            ..FakeBackend::default()
        };
        let calls = backend.calls.clone();
        let app = router().with_state(RedditClient::from_backend(backend));
        let request = Request::builder()
            .uri("/r/rust/comments/post/threads/anchor")
            .header(HOST, "localhost")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            *calls.lock().unwrap(),
            [Call::Thread {
                id: "anchor".into(),
                sort: CommentSort::Best
            }]
        );
    }
}
