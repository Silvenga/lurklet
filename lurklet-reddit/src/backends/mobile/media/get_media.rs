use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::media::MediaStream;
use crate::backends::mobile::requests::retry_after;
use crate::{Error, MediaFile, MediaKind, ResponseError, StateStore, TransportError};
use mediatype::{MediaType, names};
use std::time::Instant;
use wreq::Uri;
use wreq::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE, USER_AGENT};

const HOSTS: [&str; 9] = [
    "i.redd.it",
    "preview.redd.it",
    "external-preview.redd.it",
    "a.thumbs.redditmedia.com",
    "b.thumbs.redditmedia.com",
    "styles.redditmedia.com",
    "www.redditstatic.com",
    "packaged-media.redd.it",
    "v.redd.it",
];

impl<S: StateStore> MobileBackend<S> {
    pub(in crate::backends::mobile) async fn get_media(
        &self,
        url: &str,
    ) -> Result<MediaFile, Error> {
        let uri: Uri = url
            .parse()
            .map_err(|_| Error::InvalidRequest { field: "media_url" })?;
        let authority = uri.authority().map_or("", |authority| authority.as_str());
        let host = authority.strip_suffix(":443").unwrap_or(authority);
        if !uri
            .scheme_str()
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https"))
            || !HOSTS
                .iter()
                .any(|allowed| host.eq_ignore_ascii_case(allowed))
            || url.contains('#')
        {
            return Err(Error::InvalidRequest { field: "media_url" });
        }

        tracing::trace!("Fetching a public media file without session headers");
        let started = Instant::now();
        let response = self
            .http
            .get(uri)
            .header(ACCEPT_ENCODING, "identity")
            .header(
                USER_AGENT,
                self.config.user_agent.as_deref().unwrap_or_default(),
            )
            .send()
            .await
            .map_err(TransportError::from)?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let kind = MediaType::parse(content_type).ok().and_then(|media| {
            if media.ty == names::IMAGE && media.subty != names::_STAR {
                Some(MediaKind::Image)
            } else if media.ty == names::VIDEO
                && media.subty == names::MP4
                && media.suffix.is_none()
            {
                Some(MediaKind::Mp4)
            } else {
                None
            }
        });
        let error = if status != 200 {
            Some(Error::MediaStatus {
                status,
                retry_after: retry_after(response.headers()),
            })
        } else if kind.is_none() || response.headers().get_all(CONTENT_TYPE).iter().count() != 1 {
            Some(ResponseError::UnsupportedMediaType.into())
        } else if response
            .headers()
            .get_all(CONTENT_ENCODING)
            .iter()
            .any(|value| {
                !value
                    .to_str()
                    .is_ok_and(|value| value.eq_ignore_ascii_case("identity"))
            })
        {
            Some(ResponseError::UnsupportedMediaEncoding.into())
        } else {
            None
        };
        if let Some(error) = error {
            if self.observer.is_some() {
                self.read_response("GetMedia", response, started).await;
            }
            return Err(error);
        }

        let content_type = content_type.to_owned();
        let content_length = response.content_length();
        let kind = kind.expect("supported media type was checked");
        let body = Box::pin(MediaStream::new(
            response,
            self.observer.clone(),
            self.config.max_response_bytes,
            started,
        ));
        tracing::debug!(status, ?kind, "Media response opened");
        Ok(MediaFile {
            content_type,
            kind,
            content_length,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransportErrorKind;
    use crate::backends::mobile::testing::cached_storage;
    use crate::{
        ClientState, RedditClient, ResponseCapture, ResponseObserver, RetryAfter, StateSnapshot,
        Storage, StorageError,
    };
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use futures_util::StreamExt;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct UnusedStore;

    #[tokio::test]
    async fn when_the_total_timeout_is_disabled_then_media_should_outlive_a_previously_configured_deadline()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_raw(b"file".to_vec(), "image/png"),
            )
            .mount(&server)
            .await;
        for timeout in [Some(Duration::from_millis(5)), None] {
            let client = RedditClient::builder()
                .with_request_timeout(Duration::from_millis(5))
                .with_request_timeout(timeout)
                .with_mock_server("https://i.redd.it", &server)
                .build()
                .unwrap();

            let result = client.get_media("https://i.redd.it/file").await;

            if timeout.is_some() {
                assert!(
                    matches!(result, Err(Error::Transport(error)) if error.kind == TransportErrorKind::Timeout)
                );
            } else {
                let mut body = result.unwrap().body;
                let mut bytes = Vec::new();
                while let Some(chunk) = body.next().await {
                    bytes.extend_from_slice(&chunk.unwrap());
                }
                assert_eq!(bytes, b"file");
            }
        }
    }

    impl StateStore for UnusedStore {
        async fn load(&self) -> Result<StateSnapshot, StorageError> {
            panic!("media must not load session state")
        }

        async fn compare_exchange(
            &self,
            _: u64,
            _: ClientState,
        ) -> Result<Option<StateSnapshot>, StorageError> {
            panic!("media must not publish session state")
        }
    }

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<ResponseCapture>>>);

    impl ResponseObserver for Recorder {
        fn on_response(&self, capture: ResponseCapture) {
            self.0.lock().unwrap().push(capture);
        }
    }

    #[tokio::test]
    async fn when_an_origin_is_supported_then_media_should_use_an_unauthenticated_get_without_accessing_storage()
     {
        for host in HOSTS {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/asset.jpg"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_raw(b"file".to_vec(), "image/jpeg")
                        .insert_header("set-cookie", "cdn=must-not-be-replayed"),
                )
                .expect(2)
                .mount(&server)
                .await;
            let origin = format!("https://{host}");
            let client = RedditClient::builder()
                .with_storage(Storage::new(UnusedStore))
                .with_mock_server(&origin, &server)
                .build()
                .unwrap();

            for _ in 0..2 {
                let mut file = client
                    .get_media(&format!("{origin}/asset.jpg"))
                    .await
                    .unwrap();
                let mut bytes = Vec::new();
                while let Some(chunk) = file.body.next().await {
                    bytes.extend_from_slice(&chunk.unwrap());
                }
                assert_eq!(bytes, b"file");
            }

            let requests = server.received_requests().await.unwrap();
            assert_eq!(requests.len(), 2);
            for request in requests {
                assert!(request.body.is_empty());
                assert_eq!(request.headers[ACCEPT_ENCODING], "identity");
                assert_eq!(
                    request.headers[USER_AGENT],
                    client.backend().config.user_agent.as_deref().unwrap()
                );
                assert!(!request.headers[USER_AGENT].is_empty());
                for name in [
                    "authorization",
                    "cookie",
                    "referer",
                    "client-vendor-id",
                    "x-reddit-device-id",
                    "x-reddit-loid",
                    "x-reddit-session",
                    "x-reddit-compression",
                    "device-name",
                    "x-apollo-operation-name",
                    "x-apollo-operation-id",
                    "range",
                ] {
                    assert!(!request.headers.contains_key(name), "unexpected {name}");
                }
            }
        }
    }

    #[tokio::test]
    async fn when_the_user_agent_is_overridden_then_media_should_send_it_without_accessing_session_state()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("user-agent", "LurkletMediaTest/1.0"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(b"image".to_vec(), "image/png"))
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_user_agent("LurkletMediaTest/1.0")
            .with_storage(Storage::new(UnusedStore))
            .with_mock_server("https://external-preview.redd.it", &server)
            .build()
            .unwrap();

        let file = client
            .get_media("https://external-preview.redd.it/image")
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();

        assert_eq!(file.content_type, "image/png");
        assert_eq!(requests.len(), 1);
        for name in [
            "authorization",
            "cookie",
            "client-vendor-id",
            "x-reddit-device-id",
            "x-reddit-loid",
            "x-reddit-session",
        ] {
            assert!(!requests[0].headers.contains_key(name));
        }
    }

    #[tokio::test]
    async fn when_a_signed_url_is_requested_then_path_query_and_duplicate_parameters_should_remain_unchanged()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(b"image".to_vec(), "image/webp"))
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://preview.redd.it", &server)
            .build()
            .unwrap();
        let url = "https://preview.redd.it/a%2Fb%2fc.jpg?width=320&s=a%2Bb%2fc&tag=2&tag=1&empty=&q=a+b&q=a%20b";

        let file = client.get_media(url).await.unwrap();
        let requests = server.received_requests().await.unwrap();

        assert_eq!(file.content_type, "image/webp");
        assert_eq!(requests[0].url.path(), "/a%2Fb%2fc.jpg");
        assert_eq!(
            requests[0].url.query(),
            Some("width=320&s=a%2Bb%2fc&tag=2&tag=1&empty=&q=a+b&q=a%20b")
        );
    }

    #[tokio::test]
    async fn when_an_origin_uses_the_explicit_https_port_then_the_supported_origin_should_still_be_accepted()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(b"image".to_vec(), "image/png"))
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://I.REDD.IT:443", &server)
            .build()
            .unwrap();

        let result = client.get_media("https://I.REDD.IT:443/image").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn when_a_media_url_is_unsupported_then_validation_should_reject_it_before_network_or_state_access()
     {
        let client = RedditClient::builder()
            .with_storage(Storage::new(UnusedStore))
            .build()
            .unwrap();
        for url in [
            "http://i.redd.it/image",
            "https://i.redd.it.evil.test/image",
            "https://example.test/i.redd.it/image",
            "https://user@i.redd.it/image",
            "https://user:pass@i.redd.it/image",
            "https://i.redd.it@127.0.0.1/image",
            "https://i.redd.it:8443/image",
            "https://i.redd.it:/image",
            "https://i.redd.it:0443/image",
            "https://i.redd.it./image",
            "https://sub.i.redd.it/image",
            "https://redditstatic.com/image",
            "https://gql-fed.reddit.com/image",
            "https://127.0.0.1/image",
            "https://[::1]/image",
            "//i.redd.it/image",
            "/image",
            "file:///image",
            "data:image/png;base64,AAAA",
            "https://i.redd.it/image#fragment",
            "https://i.redd.it/image#",
            "https://i%2Eredd%2Eit/image",
            "https://i.redd.it\\@example.test/image",
            " https://i.redd.it/image",
            "https://i.redd.it/a b",
        ] {
            let result = client.get_media(url).await;

            assert!(
                matches!(result, Err(Error::InvalidRequest { field: "media_url" })),
                "{url}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn when_a_media_response_redirects_then_neither_supported_nor_external_destinations_should_be_followed()
     {
        for location in [
            "https://i.redd.it/destination",
            "https://example.test/destination",
        ] {
            for status in [301, 302, 303, 307, 308] {
                let server = MockServer::start().await;
                Mock::given(method("GET"))
                    .and(path("/redirect"))
                    .respond_with(ResponseTemplate::new(status).insert_header("location", location))
                    .expect(1)
                    .mount(&server)
                    .await;
                let client = RedditClient::builder()
                    .with_mock_server("https://i.redd.it", &server)
                    .with_mock_server("https://example.test", &server)
                    .build()
                    .unwrap();

                let result = client.get_media("https://i.redd.it/redirect").await;

                assert!(
                    matches!(result, Err(Error::MediaStatus { status: actual, .. }) if actual == status)
                );
                assert_eq!(server.received_requests().await.unwrap().len(), 1);
            }
        }
    }

    #[tokio::test]
    async fn when_media_is_not_a_complete_success_then_status_and_retry_guidance_should_not_invalidate_authentication()
     {
        for status in [204, 206, 304, 400, 401, 403, 404, 429, 500, 503] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(status)
                        .insert_header("retry-after", "60")
                        .set_body_raw(b"not a complete file".to_vec(), "image/png"),
                )
                .expect(1)
                .mount(&server)
                .await;
            let storage = cached_storage();
            let before = storage.store().load().await.unwrap();
            let client = RedditClient::builder()
                .with_storage(storage.clone())
                .with_mock_server("https://i.redd.it", &server)
                .build()
                .unwrap();

            let result = client.get_media("https://i.redd.it/asset").await;
            let after = storage.store().load().await.unwrap();

            assert!(
                matches!(result, Err(Error::MediaStatus { status: actual, retry_after: Some(RetryAfter::Delay(delay)) }) if actual == status && delay == Duration::from_secs(60))
            );
            assert_eq!(before, after);
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_a_file_has_a_supported_declaration_then_representation_should_follow_content_type_instead_of_filename_or_bytes()
     {
        for (content_type, expected) in [
            ("image/jpeg", MediaKind::Image),
            ("image/png", MediaKind::Image),
            ("IMAGE/WebP", MediaKind::Image),
            ("image/svg+xml; charset=utf-8", MediaKind::Image),
            ("image/x-future", MediaKind::Image),
            ("video/mp4; codecs=\"avc1\"", MediaKind::Mp4),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(b"not sniffed".to_vec(), content_type),
                )
                .expect(1)
                .mount(&server)
                .await;
            let client = RedditClient::builder()
                .with_mock_server("https://external-preview.redd.it", &server)
                .build()
                .unwrap();

            let file = client
                .get_media("https://external-preview.redd.it/image.gif")
                .await
                .unwrap();

            assert_eq!(
                (file.kind, file.content_type.as_str(), file.content_length),
                (expected, content_type, Some(11))
            );
        }
    }

    #[tokio::test]
    async fn when_a_type_is_missing_ambiguous_or_unsupported_then_media_should_not_infer_a_type() {
        for content_type in [
            None,
            Some("application/octet-stream"),
            Some("text/html"),
            Some("application/dash+xml"),
            Some("application/vnd.apple.mpegurl"),
            Some("application/x-mpegURL"),
            Some("video/webm"),
            Some("image/*"),
            Some("image/"),
            Some("invalid"),
            Some("image/png, text/html"),
            Some("video/mp4+xml"),
        ] {
            let server = MockServer::start().await;
            let mut reply =
                ResponseTemplate::new(200).set_body_bytes(b"\x89PNG\r\n\x1a\n".to_vec());
            if let Some(content_type) = content_type {
                reply = reply.insert_header("content-type", content_type);
            }
            Mock::given(method("GET"))
                .respond_with(reply)
                .expect(1)
                .mount(&server)
                .await;
            let client = RedditClient::builder()
                .with_mock_server("https://i.redd.it", &server)
                .build()
                .unwrap();

            let result = client.get_media("https://i.redd.it/image.png").await;

            assert!(
                matches!(
                    result,
                    Err(Error::InvalidResponse(ResponseError::UnsupportedMediaType))
                ),
                "{content_type:?}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn when_content_type_is_repeated_then_media_should_reject_the_ambiguous_declaration() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("content-type", "image/png")
                    .append_header("content-type", "text/html"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://i.redd.it", &server)
            .build()
            .unwrap();

        let result = client.get_media("https://i.redd.it/image.png").await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(ResponseError::UnsupportedMediaType))
        ));
    }

    #[tokio::test]
    async fn when_http_compression_is_returned_then_the_stream_should_supply_file_bytes_without_the_encoded_length()
     {
        let server = MockServer::start().await;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"original file bytes").unwrap();
        let encoded = encoder.finish().unwrap();
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(encoded, "image/jpeg")
                    .insert_header("content-encoding", "gzip"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://i.redd.it", &server)
            .build()
            .unwrap();

        let mut file = client.get_media("https://i.redd.it/image").await.unwrap();
        let mut bytes = Vec::new();
        while let Some(chunk) = file.body.next().await {
            bytes.extend_from_slice(&chunk.unwrap());
        }

        assert_eq!(bytes, b"original file bytes");
        assert_eq!(file.content_length, None);
        assert_eq!(file.content_type, "image/jpeg");
    }

    #[tokio::test]
    async fn when_an_encoding_cannot_be_decoded_then_media_should_not_mislabel_encoded_bytes_as_a_file()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(b"encoded".to_vec(), "image/png")
                    .insert_header("content-encoding", "unknown"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = RedditClient::builder()
            .with_mock_server("https://i.redd.it", &server)
            .build()
            .unwrap();

        let result = client.get_media("https://i.redd.it/image").await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(
                ResponseError::UnsupportedMediaEncoding
            ))
        ));
    }

    #[tokio::test]
    async fn when_a_file_exceeds_the_buffer_limit_then_download_should_continue_with_only_a_bounded_capture()
     {
        let server = MockServer::start().await;
        let expected = vec![42; 2 * 1024 * 1024];
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(expected.clone(), "video/mp4"))
            .expect(1)
            .mount(&server)
            .await;
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_mock_server("https://v.redd.it", &server)
            .with_max_response_bytes(16)
            .with_response_observer(recorder.clone())
            .build()
            .unwrap();

        let mut file = client
            .get_media("https://v.redd.it/direct.mp4?s=opaque")
            .await
            .unwrap();
        assert!(recorder.0.lock().unwrap().is_empty());
        drop(client);
        let mut received = Vec::new();
        while let Some(chunk) = file.body.next().await {
            received.extend_from_slice(&chunk.unwrap());
        }
        drop(file);

        assert_eq!(received, expected);
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].operation, "GetMedia");
        assert_eq!(captures[0].body, expected[..16]);
        assert!(captures[0].body_truncated);
        assert!(captures[0].body_error.is_none());
    }

    #[tokio::test]
    async fn when_a_rejected_response_is_captured_then_its_diagnostic_body_should_be_bounded_and_not_returned_as_media()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_raw(b"<html>denied</html>".to_vec(), "text/html"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let recorder = Recorder::default();
        let client = RedditClient::builder()
            .with_mock_server("https://preview.redd.it", &server)
            .with_max_response_bytes(6)
            .with_response_observer(recorder.clone())
            .build()
            .unwrap();

        let result = client
            .get_media("https://preview.redd.it/image.jpg?s=invalid")
            .await;

        assert!(matches!(
            result,
            Err(Error::MediaStatus { status: 403, .. })
        ));
        let captures = recorder.0.lock().unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].body, b"<html>");
        assert!(captures[0].body_truncated);
    }
}
