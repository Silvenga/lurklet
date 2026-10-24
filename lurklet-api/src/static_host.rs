use crate::middleware::{access_log, static_policy};
use crate::routes::robots;
use anyhow::{Context, ensure};
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use mediatype::{MediaTypeList, ReadParams, names};
use percent_encoding::percent_decode_str;
use std::path::{Path, PathBuf};
use tokio::fs;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

/// Serves a read-only website independently of the API namespace.
pub struct StaticHost {
    root: PathBuf,
}

impl StaticHost {
    /// Validates the website root and required entry document.
    pub async fn new(directory: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = fs::canonicalize(directory)
            .await
            .context("static directory is unavailable")?;
        let file = fs::File::open(root.join("index.html"))
            .await
            .context("static index.html is missing or unreadable")?;
        ensure!(
            file.metadata().await?.is_file(),
            "static index.html is not a file"
        );
        Ok(Self { root })
    }

    /// Builds a fallback router without exposing filesystem paths to clients.
    pub fn router(self) -> Router {
        let index = ServeFile::new(self.root.join("index.html"))
            .precompressed_br()
            .precompressed_gzip();
        let files = ServeDir::new(self.root)
            .append_index_html_on_directories(false)
            .precompressed_br()
            .precompressed_gzip()
            .fallback(get(spa_fallback).with_state(index.clone()));

        Router::new()
            .route("/robots.txt", get(robots))
            .route_service("/", index)
            .fallback_service(files)
            .layer(middleware::from_fn(static_policy))
            .layer(middleware::from_fn(access_log))
    }
}

async fn spa_fallback(State(index): State<ServeFile>, request: Request) -> Response {
    let path = percent_decode_str(request.uri().path()).decode_utf8_lossy();
    let reserved = path == "/assets"
        || path.starts_with("/assets/")
        || path.starts_with("/favicon")
        || path.starts_with("/robots.txt/")
        || mime_guess::from_path(&*path)
            .first_raw()
            .is_some_and(|mime| mime != "text/html");
    let response = if !reserved && accepts_html(request.headers()) {
        index.oneshot(request).await.unwrap().map(Body::new)
    } else {
        StatusCode::NOT_FOUND.into_response()
    };
    ([(header::VARY, "Accept")], response).into_response()
}

fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(MediaTypeList::new)
        .filter_map(Result::ok)
        .any(|media| {
            media.ty == names::TEXT
                && media.subty == names::HTML
                && media.suffix.is_none()
                && media.get_param(names::Q).is_none_or(|value| {
                    value
                        .as_str()
                        .parse::<f32>()
                        .is_ok_and(|quality| quality > 0.0 && quality <= 1.0)
                })
        })
}

#[cfg(test)]
mod tests {
    use super::StaticHost;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use brotli::{CompressorWriter, Decompressor};
    use flate2::Compression;
    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::{Read, Write};
    use tower::ServiceExt;

    const ROBOTS: &str = "User-agent: *\nDisallow: /\n";
    const HTML: &str = "<!doctype html><html><body>Static fixture</body></html>";
    const CSS: &str = "body { color: black; background-color: white; font-family: sans-serif; }";

    fn fixtures() -> TempDir {
        let directory = TempDir::new().unwrap();
        directory.child("assets").create_dir_all().unwrap();
        for (path, body) in [("index.html", HTML), ("assets/main-abc12345.css", CSS)] {
            directory.child(path).write_str(body).unwrap();
            let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
            gzip.write_all(body.as_bytes()).unwrap();
            directory
                .child(format!("{path}.gz"))
                .write_binary(&gzip.finish().unwrap())
                .unwrap();
            let mut brotli = Vec::new();
            {
                let mut writer = CompressorWriter::new(&mut brotli, 4096, 5, 22);
                writer.write_all(body.as_bytes()).unwrap();
            }
            directory
                .child(format!("{path}.br"))
                .write_binary(&brotli)
                .unwrap();
        }
        directory.child("plain.txt").write_str("plain").unwrap();
        directory
            .child("favicon.ico")
            .write_binary(b"icon")
            .unwrap();
        directory
            .child("robots.txt")
            .write_str("User-agent: *\nAllow: /\n")
            .unwrap();
        directory
    }

    async fn app(directory: &TempDir) -> Router {
        StaticHost::new(directory.path()).await.unwrap().router()
    }

    #[tokio::test]
    async fn when_precompressed_variants_exist_then_get_and_head_should_select_the_correct_bytes() {
        let directory = fixtures();
        let app = app(&directory).await;

        for (accept, expected) in [
            ("", None),
            ("gzip", Some("gzip")),
            ("br", Some("br")),
            ("gzip, br", Some("br")),
            ("br;q=0.1, gzip;q=0.9", Some("gzip")),
            ("br;q=0, gzip;q=1", Some("gzip")),
            ("br;q=0.2, identity;q=1", None),
        ] {
            for method in [Method::GET, Method::HEAD] {
                let request = Request::builder()
                    .method(method.clone())
                    .uri("/assets/main-abc12345.css?version=1")
                    .header(header::ACCEPT_ENCODING, accept)
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(request).await.unwrap();
                let status = response.status();
                let headers = response.headers().clone();
                let body = to_bytes(response.into_body(), 4096).await.unwrap();

                assert_eq!(status, StatusCode::OK, "{method} {accept}");
                assert_eq!(headers[header::CONTENT_TYPE], "text/css");
                assert_eq!(
                    headers[header::CACHE_CONTROL],
                    "public, max-age=31536000, immutable"
                );
                assert_eq!(headers[header::VARY], "Accept-Encoding");
                assert_eq!(
                    headers
                        .get(header::CONTENT_ENCODING)
                        .map(|value| value.to_str().unwrap()),
                    expected
                );
                let path = match expected {
                    Some("br") => "assets/main-abc12345.css.br",
                    Some("gzip") => "assets/main-abc12345.css.gz",
                    None => "assets/main-abc12345.css",
                    Some(other) => panic!("unexpected encoding: {other}"),
                };
                let expected_bytes = fs::read(directory.child(path).path()).unwrap();
                assert_eq!(
                    headers[header::CONTENT_LENGTH],
                    expected_bytes.len().to_string()
                );
                if method == Method::HEAD {
                    assert!(body.is_empty());
                } else {
                    assert_eq!(body.as_ref(), expected_bytes);
                    let mut decoded = String::new();
                    match expected {
                        Some("br") => {
                            Decompressor::new(&body[..], 4096)
                                .read_to_string(&mut decoded)
                                .unwrap();
                        }
                        Some("gzip") => {
                            GzDecoder::new(&body[..])
                                .read_to_string(&mut decoded)
                                .unwrap();
                        }
                        None => decoded = String::from_utf8(body.to_vec()).unwrap(),
                        Some(other) => panic!("unexpected encoding: {other}"),
                    }
                    assert_eq!(decoded, CSS);
                }
            }
        }
    }

    #[tokio::test]
    async fn when_variants_are_missing_then_selection_should_fall_back_to_available_files() {
        let directory = fixtures();
        directory
            .child("assets/gzip-only-abc12345.css")
            .write_str(CSS)
            .unwrap();
        let compressed = fs::read(directory.child("assets/main-abc12345.css.gz").path()).unwrap();
        directory
            .child("assets/gzip-only-abc12345.css.gz")
            .write_binary(&compressed)
            .unwrap();
        let app = app(&directory).await;

        for (path, accept, status, encoding) in [
            (
                "/assets/gzip-only-abc12345.css",
                "br, gzip, identity;q=0",
                StatusCode::OK,
                Some("gzip"),
            ),
            ("/plain.txt", "br, gzip", StatusCode::OK, None),
        ] {
            let request = Request::builder()
                .uri(path)
                .header(header::ACCEPT_ENCODING, accept)
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), status, "{path} {accept}");
            assert_eq!(
                response
                    .headers()
                    .get(header::CONTENT_ENCODING)
                    .map(|value| value.to_str().unwrap()),
                encoding
            );
        }
    }

    #[tokio::test]
    async fn when_html_navigation_misses_a_file_then_response_should_rewrite_without_redirecting() {
        let directory = fixtures();
        let app = app(&directory).await;

        for path in [
            "/r/rust",
            "/r/rust/",
            "/posts/version.2",
            "/missing.html?query=value",
        ] {
            let request = Request::builder()
                .uri(path)
                .header(header::ACCEPT, "application/json, text/html;q=0.8")
                .header(header::ACCEPT_ENCODING, "br")
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            let mut decoded = String::new();
            Decompressor::new(&body[..], 4096)
                .read_to_string(&mut decoded)
                .unwrap();

            assert_eq!(status, StatusCode::OK, "{path}");
            assert_eq!(headers[header::CONTENT_TYPE], "text/html");
            assert_eq!(headers[header::CACHE_CONTROL], "public, no-cache");
            assert_eq!(headers[header::VARY], "Accept, Accept-Encoding");
            assert!(!headers.contains_key(header::LOCATION));
            assert_eq!(decoded, HTML);
        }
    }

    #[tokio::test]
    async fn when_navigation_is_ineligible_or_path_is_reserved_then_response_should_not_be_html() {
        let directory = fixtures();
        let app = app(&directory).await;

        for (path, accept) in [
            ("/unknown", "*/*"),
            ("/unknown", "application/json"),
            ("/unknown", "text/html;q=0, */*"),
            ("/unknown", "text/html;q=invalid"),
            ("/unknown", "text/*"),
            ("/unknown", "application/json; note=\"a,text/html,b\""),
            ("/assets", "text/html"),
            ("/assets/missing.css", "text/html"),
            ("/favicon.svg", "text/html"),
            ("/missing.js", "text/html"),
            ("/manifest.webmanifest", "text/html"),
            ("/api", "text/html"),
            ("/api/", "text/html"),
            ("/api/missing", "text/html"),
            ("/%61pi/missing", "text/html"),
        ] {
            let request = Request::builder()
                .uri(path)
                .header(header::ACCEPT, accept)
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 4096).await.unwrap();

            assert_eq!(status, StatusCode::NOT_FOUND, "{path} {accept}");
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert!(!body.starts_with(b"<!doctype html>"));
        }
    }

    #[tokio::test]
    async fn when_files_are_unversioned_then_responses_should_require_revalidation() {
        let directory = fixtures();
        let app = app(&directory).await;

        for path in ["/", "/index.html", "/plain.txt", "/favicon.ico"] {
            let request = Request::builder().uri(path).body(Body::empty()).unwrap();

            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, no-cache"
            );
        }
    }

    #[tokio::test]
    async fn when_a_validator_matches_then_not_modified_should_preserve_variant_and_cache_headers()
    {
        let directory = fixtures();
        let app = app(&directory).await;

        for path in ["/index.html", "/r/rust", "/assets/main-abc12345.css"] {
            let mut tags = Vec::new();
            for encoding in ["identity", "gzip, identity;q=0", "br, identity;q=0"] {
                let request = Request::builder()
                    .uri(path)
                    .header(header::ACCEPT, "text/html")
                    .header(header::ACCEPT_ENCODING, encoding)
                    .body(Body::empty())
                    .unwrap();
                let initial = app.clone().oneshot(request).await.unwrap();
                let headers = initial.headers().clone();
                let etag = headers[header::ETAG].clone();
                let request = Request::builder()
                    .uri(path)
                    .header(header::ACCEPT, "text/html")
                    .header(header::ACCEPT_ENCODING, encoding)
                    .header(
                        header::IF_NONE_MATCH,
                        format!("\"unrelated\", W/{}", etag.to_str().unwrap()),
                    )
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(request).await.unwrap();
                let status = response.status();
                let cached_headers = response.headers().clone();
                let body = to_bytes(response.into_body(), 4096).await.unwrap();

                assert_eq!(status, StatusCode::NOT_MODIFIED, "{path} {encoding}");
                for name in [header::ETAG, header::CACHE_CONTROL, header::VARY] {
                    assert_eq!(cached_headers.get(&name), headers.get(&name), "{name}");
                }
                assert!(body.is_empty());
                assert!(!tags.contains(&etag));
                tags.push(etag);
            }
        }
    }

    #[tokio::test]
    async fn when_robots_is_requested_then_generated_denial_should_override_disk_content() {
        let directory = fixtures();
        let app = app(&directory).await;

        for path in ["/robots.txt", "/%72obots.txt?any=value"] {
            for method in [Method::GET, Method::HEAD] {
                let request = Request::builder()
                    .uri(path)
                    .method(method.clone())
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(request).await.unwrap();
                let status = response.status();
                let headers = response.headers().clone();
                let body = to_bytes(response.into_body(), 4096).await.unwrap();

                assert_eq!(status, StatusCode::OK);
                assert_eq!(headers[header::CONTENT_TYPE], "text/plain; charset=utf-8");
                assert_eq!(headers[header::CACHE_CONTROL], "public, no-cache");
                assert_eq!(headers[header::CONTENT_LENGTH], ROBOTS.len().to_string());
                assert_eq!(
                    body.as_ref(),
                    if method == Method::HEAD {
                        b""
                    } else {
                        ROBOTS.as_bytes()
                    }
                );
            }
        }
    }

    #[tokio::test]
    async fn when_static_method_is_unsupported_then_response_should_be_uncacheable_method_not_allowed()
     {
        let directory = fixtures();
        let app = app(&directory).await;

        for path in ["/", "/index.html", "/robots.txt", "/missing"] {
            for method in [Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS] {
                let request = Request::builder()
                    .uri(path)
                    .method(method)
                    .header(header::ACCEPT, "text/html")
                    .body(Body::empty())
                    .unwrap();

                let response = app.clone().oneshot(request).await.unwrap();

                assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
                assert_eq!(response.headers()[header::ALLOW], "GET,HEAD");
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            }
        }
    }

    #[tokio::test]
    async fn when_path_is_private_or_traverses_directories_then_response_should_be_not_found() {
        let directory = fixtures();
        directory.child(".env").write_str("private").unwrap();
        let app = app(&directory).await;

        for path in [
            "/../Cargo.toml",
            "/%2e%2e/Cargo.toml",
            "/assets/../index.html",
            "/foo%5cbar",
            "/file%00",
            "/.env",
            "/%2eenv",
            "/.vite/manifest.json",
            "/index.html.gz",
        ] {
            let request = Request::builder()
                .uri(path)
                .header(header::ACCEPT, "text/html")
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert!(
                to_bytes(response.into_body(), 4096)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn when_index_is_missing_or_a_directory_then_initialization_should_fail() {
        let directory = TempDir::new().unwrap();

        let missing = StaticHost::new(directory.path()).await;
        directory.child("index.html").create_dir_all().unwrap();
        let not_a_file = StaticHost::new(directory.path()).await;

        assert!(missing.is_err());
        assert!(not_a_file.is_err());
    }
}
