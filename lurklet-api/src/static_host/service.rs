use super::negotiation::{Encoding, accepted_encodings, accepts_html};
use anyhow::{Context, ensure};
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use percent_encoding::percent_decode_str;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::fs;
use tower_http::services::ServeFile;
use tracing::Instrument;

const ROBOTS: &str = "User-agent: *\nDisallow: /\n";

/// Serves a read-only website independently of the API namespace.
#[derive(Clone)]
pub struct StaticHost {
    root: Arc<Path>,
}

impl StaticHost {
    /// Validates the website root and required entry document.
    pub async fn new(directory: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = fs::canonicalize(directory)
            .await
            .context("static directory is unavailable")?;
        let host = Self { root: root.into() };
        let index = host
            .resolve("index.html")
            .await?
            .context("static index.html is missing")?;
        let file = fs::File::open(index)
            .await
            .context("static index.html is unreadable")?;
        ensure!(
            file.metadata().await?.is_file(),
            "static index.html is not a file"
        );
        Ok(host)
    }

    /// Builds a fallback router without exposing filesystem paths to clients.
    pub fn router(self) -> Router {
        Router::new().fallback(Self::serve).with_state(self)
    }

    async fn resolve(&self, relative: &str) -> io::Result<Option<PathBuf>> {
        match fs::canonicalize(self.root.join(relative)).await {
            Ok(path) if path.starts_with(&*self.root) => {
                Ok(fs::metadata(&path).await?.is_file().then_some(path))
            }
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "static path leaves configured directory",
            )),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    async fn serve(State(host): State<Self>, mut request: Request) -> Response {
        let span = tracing::debug_span!("http.static", method = %request.method(), path = request.uri().path_and_query().map_or("/", |path| path.as_str()));
        async move {
            let started = Instant::now();
            tracing::trace!(headers = ?request.headers(), "HTTP request metadata");
            let mut cache_control = "no-store";
            let mut vary_accept = false;
            let result: io::Result<Response> = async {
                if request.method() != Method::GET && request.method() != Method::HEAD {
                    return Ok((
                        StatusCode::METHOD_NOT_ALLOWED,
                        [(header::ALLOW, "GET,HEAD")],
                    )
                        .into_response());
                }
                let Ok(decoded) = percent_decode_str(request.uri().path()).decode_utf8() else {
                    return Ok(StatusCode::NOT_FOUND.into_response());
                };
                let path = decoded.trim_start_matches('/').to_owned();
                if decoded.contains(['\\', '\0'])
                    || path.split('/').any(|part| part.starts_with('.'))
                    || path == "api"
                    || path.starts_with("api/")
                    || path.ends_with(".br")
                    || path.ends_with(".gz")
                {
                    return Ok(StatusCode::NOT_FOUND.into_response());
                }
                let encodings = accepted_encodings(request.headers());
                if path == "robots.txt" {
                    if !encodings.contains(&Encoding::Identity) {
                        return Ok(StatusCode::NOT_ACCEPTABLE.into_response());
                    }
                    cache_control = "public, no-cache";
                    let body = if request.method() == Method::HEAD {
                        ""
                    } else {
                        ROBOTS
                    };
                    let mut response = body.into_response();
                    response
                        .headers_mut()
                        .insert(header::CONTENT_LENGTH, HeaderValue::from(ROBOTS.len()));
                    return Ok(response);
                }
                let mut relative = if path.is_empty() {
                    "index.html".to_owned()
                } else {
                    path
                };
                if host.resolve(&relative).await?.is_none() {
                    let reserved = relative == "assets"
                        || relative.starts_with("assets/")
                        || relative.starts_with("favicon")
                        || relative.starts_with("robots.txt/")
                        || matches!(
                            Path::new(&relative)
                                .extension()
                                .and_then(|extension| extension.to_str()),
                            Some(
                                "js" | "mjs"
                                    | "css"
                                    | "json"
                                    | "txt"
                                    | "ico"
                                    | "svg"
                                    | "png"
                                    | "jpg"
                                    | "jpeg"
                                    | "gif"
                                    | "webp"
                                    | "avif"
                                    | "woff"
                                    | "woff2"
                                    | "ttf"
                                    | "wasm"
                                    | "map"
                                    | "webmanifest"
                            )
                        );
                    vary_accept = !reserved;
                    if reserved || !accepts_html(request.headers()) {
                        return Ok(StatusCode::NOT_FOUND.into_response());
                    }
                    relative = "index.html".to_owned();
                }
                let mut selected = None;
                for encoding in encodings {
                    if let Some(file) = host
                        .resolve(&format!("{relative}{}", encoding.suffix()))
                        .await?
                    {
                        selected = Some((file, encoding));
                        break;
                    }
                }
                let Some((file, encoding)) = selected else {
                    return Ok(StatusCode::NOT_ACCEPTABLE.into_response());
                };
                let mime = mime_guess::from_path(&relative).first_or_octet_stream();
                cache_control =
                    if relative.starts_with("assets/") && mime.essence_str() != "text/html" {
                        "public, max-age=31536000, immutable"
                    } else {
                        "public, no-cache"
                    };
                request.headers_mut().remove(header::ACCEPT_ENCODING);
                // ServeFile does not implement If-Range. A full response is safe in that case.
                if request.headers().contains_key(header::IF_RANGE) {
                    request.headers_mut().remove(header::RANGE);
                }
                let mut response = ServeFile::new_with_mime(file, &mime)
                    .try_call(request)
                    .await?
                    .map(Body::new);
                if (response.status().is_success() || response.status() == StatusCode::NOT_MODIFIED)
                    && let Some(encoding) = encoding.content_encoding()
                {
                    response
                        .headers_mut()
                        .insert(header::CONTENT_ENCODING, HeaderValue::from_static(encoding));
                }
                Ok(response)
            }
            .await;
            let mut response = match result {
                Ok(response) => response,
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    StatusCode::NOT_FOUND.into_response()
                }
                Err(error) => {
                    tracing::warn!("Unable to serve a static file");
                    tracing::debug!(?error, "Static file failure details");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            };
            if response.status().is_client_error() || response.status().is_server_error() {
                cache_control = "no-store";
            }
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static(cache_control),
            );
            response.headers_mut().insert(
                header::VARY,
                HeaderValue::from_static(if vary_accept {
                    "Accept, Accept-Encoding"
                } else {
                    "Accept-Encoding"
                }),
            );
            tracing::debug!(
                status = response.status().as_u16(),
                duration_ms = started.elapsed().as_secs_f64() * 1000.0,
                "HTTP request completed"
            );
            tracing::trace!(headers = ?response.headers(), "HTTP response metadata");
            response
        }
        .instrument(span)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::{ROBOTS, StaticHost};
    use crate::application;
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
        let (app, _) = application();
        app.fallback_service(StaticHost::new(directory.path()).await.unwrap().router())
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
            ("*;q=1, br;q=0", Some("gzip")),
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
    async fn when_variants_are_missing_then_selection_should_fall_back_only_to_acceptable_encodings()
     {
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
            (
                "/plain.txt",
                "br, identity;q=0",
                StatusCode::NOT_ACCEPTABLE,
                None,
            ),
            (
                "/assets/main-abc12345.css",
                "*;q=0",
                StatusCode::NOT_ACCEPTABLE,
                None,
            ),
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
            if status == StatusCode::NOT_ACCEPTABLE {
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                assert!(
                    to_bytes(response.into_body(), 4096)
                        .await
                        .unwrap()
                        .is_empty()
                );
            }
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
            for encoding in ["identity", "gzip", "br"] {
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
                for name in [
                    header::ETAG,
                    header::CACHE_CONTROL,
                    header::VARY,
                    header::CONTENT_ENCODING,
                ] {
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

    #[cfg(unix)]
    #[tokio::test]
    async fn when_symlink_escapes_static_directory_then_response_should_be_not_found() {
        use std::os::unix::fs::symlink;

        let directory = fixtures();
        let outside = TempDir::new().unwrap();
        outside.child("secret").write_str("private").unwrap();
        symlink(outside.path(), directory.child("outside").path()).unwrap();
        let app = app(&directory).await;
        let request = Request::builder()
            .uri("/outside/secret")
            .header(header::ACCEPT, "text/html")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            to_bytes(response.into_body(), 4096)
                .await
                .unwrap()
                .is_empty()
        );
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
