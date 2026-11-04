use crate::response_cache::CacheKey;
use axum::Router;
use axum::extract::{MatchedPath, OriginalUri, Request, State};
use axum::http::Uri;
use axum::middleware::{self, Next};
use axum::response::Response;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use utoipa::openapi::path::ParameterIn;
use utoipa::openapi::{OpenApi, RefOr};

// Image keys come from captured previews, live icons/posters, and these examples:
// https://github.com/glanceapp/glance/issues/1016 (height)
// https://github.com/uBlockOrigin/uAssets/issues/17225 (blur)
// https://www.reddit.com/r/help/comments/uyw5xq/ (crop)
// MP4 parameters: https://github.com/yshui/picom/issues/1467 (m, v, e)
// and https://github.com/QuantumBadger/RedReader/issues/752 (source).
const MEDIA_CACHE_KEYS: &[&str] = &[
    "auto", "blur", "crop", "e", "format", "frame", "height", "m", "source", "v", "width",
];
const MAX_DROPPED_KEYS: usize = 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueryKeys {
    cache_key: Vec<String>,
    forward_only: Vec<String>,
}

/// Per-route query allowlists shared by request filtering and CDN configuration exports.
#[derive(Serialize)]
pub struct QueryPolicy {
    #[serde(flatten)]
    routes: BTreeMap<String, QueryKeys>,
    #[serde(skip)]
    dropped: Mutex<HashSet<blake3::Hash>>,
}

impl QueryPolicy {
    /// Derives data keys from GET parameters and adds the upstream media contract.
    pub fn from_specification(specification: &OpenApi) -> Self {
        let routes = specification
            .paths
            .paths
            .iter()
            .filter_map(|(path, item)| {
                let operation = item.get.as_ref()?;
                let keys = if operation.operation_id.as_deref() == Some("get_media") {
                    QueryKeys {
                        cache_key: MEDIA_CACHE_KEYS.iter().map(|key| (*key).into()).collect(),
                        forward_only: vec!["s".into()],
                    }
                } else {
                    let mut keys = item
                        .parameters
                        .iter()
                        .flatten()
                        .chain(operation.parameters.iter().flatten())
                        .filter_map(|parameter| match parameter {
                            RefOr::T(parameter) if parameter.parameter_in == ParameterIn::Query => {
                                Some(parameter.name.clone())
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    keys.sort();
                    keys.dedup();
                    QueryKeys {
                        cache_key: keys,
                        forward_only: Vec::new(),
                    }
                };
                Some((path.clone(), keys))
            })
            .collect();
        Self {
            routes,
            dropped: Mutex::default(),
        }
    }

    /// Filters requests before persistent caching, retaining the original URI for access logging.
    pub fn wrap(self, routes: Router) -> Router {
        routes.layer(middleware::from_fn_with_state(Arc::new(self), Self::apply))
    }

    async fn apply(State(policy): State<Arc<Self>>, mut request: Request, next: Next) -> Response {
        let Some(path) = request.extensions().get::<MatchedPath>() else {
            return next.run(request).await;
        };
        // The media route uses an Axum catch-all but a regular OpenAPI path parameter.
        let path = path.as_str().replace("{*path}", "{path}");
        let Some(keys) = policy.routes.get(&path) else {
            return next.run(request).await;
        };
        let mut forwarded = Vec::new();
        let mut cached = Vec::new();
        let mut stripped = false;
        for pair in request.uri().query().unwrap_or_default().split('&') {
            let Some((key, _)) = form_urlencoded::parse(pair.as_bytes()).next() else {
                continue;
            };
            if keys.cache_key.iter().any(|allowed| allowed == &key) {
                forwarded.push(pair);
                cached.push(pair);
            } else if keys.forward_only.iter().any(|allowed| allowed == &key) {
                forwarded.push(pair);
            } else {
                stripped = true;
                let mut hasher = blake3::Hasher::new();
                hasher.update(path.as_bytes());
                hasher.update(b"?");
                hasher.update(key.as_bytes());
                let digest = hasher.finalize();
                let mut dropped = policy.dropped.lock().unwrap();
                if dropped.len() < MAX_DROPPED_KEYS && dropped.insert(digest) {
                    let key = key.chars().take(128).collect::<String>();
                    tracing::warn!(parent: None, route = %path, query_key = ?key, "Dropped an unknown query key");
                    if dropped.len() == MAX_DROPPED_KEYS {
                        tracing::warn!(
                            parent: None,
                            "Dropped-query discovery limit reached; further key warnings are suppressed until restart"
                        );
                    }
                }
            }
        }
        let original = request
            .extensions()
            .get::<OriginalUri>()
            .map_or(request.uri(), |uri| &uri.0);
        let cache_uri = with_query(original, &cached.join("&"));
        let forwarded_uri = stripped.then(|| with_query(request.uri(), &forwarded.join("&")));
        request.extensions_mut().insert(CacheKey::new(&cache_uri));
        if let Some(uri) = forwarded_uri {
            *request.uri_mut() = uri;
        }
        next.run(request).await
    }
}

fn with_query(uri: &Uri, query: &str) -> Uri {
    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(if query.is_empty() {
        uri.path().parse().unwrap()
    } else {
        format!("{}?{query}", uri.path()).parse().unwrap()
    });
    Uri::from_parts(parts).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::testing::{Call, FakeBackend};
    use crate::{ApiCacheOptions, RateLimits, ResponseCache, application, specification};
    use axum::Json;
    use axum::body::{Body, to_bytes};
    use axum::extract::ConnectInfo;
    use axum::http::{Method, StatusCode};
    use axum::routing::get;
    use lurklet_reddit::{CommentForest, CommentSort, RedditClient};
    use serde_json::{Value, json};
    use std::net::SocketAddr;
    use tower::ServiceExt;

    #[test]
    fn when_query_policy_is_exported_then_it_should_include_route_specific_keys_and_forward_only_signatures()
     {
        let policy = QueryPolicy::from_specification(&specification());

        let exported = serde_json::to_value(policy).unwrap();

        assert_eq!(
            exported,
            json!({
                "/api/health": {"cacheKey": [], "forwardOnly": []},
                "/api/posts/{id}": {"cacheKey": [], "forwardOnly": []},
                "/api/posts/{id}/comments": {"cacheKey": ["cursor", "sort"], "forwardOnly": []},
                "/api/comments/{id}/thread": {"cacheKey": ["sort"], "forwardOnly": []},
                "/api/subreddits/{subreddit}": {"cacheKey": [], "forwardOnly": []},
                "/api/subreddits/{subreddit}/posts": {"cacheKey": ["after", "sort", "timeRange"], "forwardOnly": []},
                "/api/media/{origin}/{path}": {
                    "cacheKey": ["auto", "blur", "crop", "e", "format", "frame", "height", "m", "source", "v", "width"],
                    "forwardOnly": ["s"]
                }
            })
        );
    }

    #[tokio::test]
    async fn when_unknown_keys_are_stripped_then_forwarded_bytes_and_original_uri_should_be_preserved()
     {
        let mut routes = Router::new();
        for path in [
            "/posts/{id}",
            "/posts/{id}/comments",
            "/comments/{id}/thread",
            "/subreddits/{subreddit}/posts",
            "/media/{origin}/{*path}",
        ] {
            routes = routes.route(path, get(|request: Request| async move {
                Json(json!({
                    "uri": request.uri().to_string(),
                    "original": request.extensions().get::<OriginalUri>().unwrap().0.to_string(),
                    "cache": request.extensions().get::<CacheKey>().unwrap().uri().to_string(),
                }))
            }));
        }
        let app = Router::new().nest_service(
            "/api",
            QueryPolicy::from_specification(&specification()).wrap(routes),
        );

        for (input, forwarded, cache) in [
            ("/posts/post?sort=new&_=1", "/posts/post", "/posts/post"),
            (
                "/comments/comment/thread?sort=top&cursor=ignored",
                "/comments/comment/thread?sort=top",
                "/comments/comment/thread?sort=top",
            ),
            (
                "/posts/post/comments?%73ort=new&cursor=a%2Bb%20c&junk=ignored",
                "/posts/post/comments?%73ort=new&cursor=a%2Bb%20c",
                "/posts/post/comments?cursor=a%2Bb+c&sort=new",
            ),
            (
                "/posts/post/comments?sort=new&sort=top&junk=ignored",
                "/posts/post/comments?sort=new&sort=top",
                "/posts/post/comments?sort=new&sort=top",
            ),
            (
                "/subreddits/rust/posts?sort=top&timeRange=week&after=opaque%2Btoken&cursor=ignored",
                "/subreddits/rust/posts?sort=top&timeRange=week&after=opaque%2Btoken",
                "/subreddits/rust/posts?after=opaque%2Btoken&sort=top&timeRange=week",
            ),
            (
                "/media/preview.redd.it/a%2Fb%20c.png?width=640&x=1&auto=webp&s=a%2Bb+z",
                "/media/preview.redd.it/a%2Fb%20c.png?width=640&auto=webp&s=a%2Bb+z",
                "/media/preview.redd.it/a%2Fb%20c.png?auto=webp&width=640",
            ),
            (
                "/media/preview.redd.it/file.png?%73=first&s=second&width=320&width=640&WIDTH=99",
                "/media/preview.redd.it/file.png?%73=first&s=second&width=320&width=640",
                "/media/preview.redd.it/file.png?width=320&width=640",
            ),
            (
                "/media/i.redd.it/file.png?",
                "/media/i.redd.it/file.png?",
                "/media/i.redd.it/file.png",
            ),
            (
                "/media/packaged-media.redd.it/file.mp4?m=DASHPlaylist.mpd&v=1&e=123&s=signed&unknown=x",
                "/media/packaged-media.redd.it/file.mp4?m=DASHPlaylist.mpd&v=1&e=123&s=signed",
                "/media/packaged-media.redd.it/file.mp4?e=123&m=DASHPlaylist.mpd&v=1",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api{input}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({
                    "uri": forwarded,
                    "original": format!("/api{input}"),
                    "cache": format!("/api{cache}")
                }),
                "{input}"
            );
        }
    }

    #[tokio::test]
    async fn when_data_queries_vary_then_only_accepted_keys_should_change_persistent_cache_identity()
     {
        let directory = assert_fs::TempDir::new().unwrap();
        let cache = ResponseCache::new(directory.path()).await.unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let reddit = RedditClient::from_backend(FakeBackend {
            comments: Some(Ok(CommentForest {
                applied_sort: None,
                items: Vec::new(),
            })),
            calls: calls.clone(),
            ..FakeBackend::default()
        });
        let app = application(
            reddit,
            &RateLimits::default(),
            Some(&cache),
            &ApiCacheOptions::default(),
        );

        for (method, query, status, outcome) in [
            (Method::GET, "sort=new&random=one", StatusCode::OK, "miss"),
            (Method::GET, "another=two&%73ort=new", StatusCode::OK, "hit"),
            (Method::HEAD, "sort=new&third=three", StatusCode::OK, "hit"),
            (Method::GET, "sort=top&random=four", StatusCode::OK, "miss"),
            (
                Method::GET,
                "sort=new&cursor=opaque",
                StatusCode::OK,
                "miss",
            ),
            (
                Method::GET,
                "sort=new&sort=top",
                StatusCode::BAD_REQUEST,
                "miss",
            ),
        ] {
            let request = Request::builder()
                .method(method.clone())
                .uri(format!("/api/posts/post/comments?{query}"))
                .extension(ConnectInfo("192.0.2.1:1234".parse::<SocketAddr>().unwrap()))
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), status, "{query}");
            assert_eq!(response.headers()["x-lurklet-cache"], outcome, "{query}");
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            assert_eq!(body.is_empty(), method == Method::HEAD);
        }
        assert_eq!(
            *calls.lock().unwrap(),
            [
                Call::Comments {
                    id: "post".into(),
                    sort: CommentSort::New,
                    cursor: None
                },
                Call::Comments {
                    id: "post".into(),
                    sort: CommentSort::Top,
                    cursor: None
                },
                Call::Comments {
                    id: "post".into(),
                    sort: CommentSort::New,
                    cursor: Some("opaque".into())
                },
            ]
        );
    }
}
