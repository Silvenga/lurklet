use crate::backends::mobile::MobileBackend;
use crate::backends::mobile::graphql::{PersistedQuery, decode_response};
use crate::backends::mobile::subreddit_feeds::{Continuation, FeedData, Variables};
use crate::{Error, ResponseError, StateStore, SubredditPage, SubredditSort};
use serde_json::json;
use uuid::Uuid;

impl<S: StateStore> MobileBackend<S> {
    pub(in crate::backends::mobile) async fn list_subreddit_posts(
        &self,
        subreddit: &str,
        sort: SubredditSort,
        after: Option<&str>,
    ) -> Result<SubredditPage, Error> {
        let previous = after
            .map(|token| Continuation::decode(token, subreddit, sort))
            .transpose()?;
        let navigation_session_id = previous
            .as_ref()
            .map_or_else(Uuid::new_v4, |cursor| cursor.navigation_session_id);
        let after = previous.as_ref().map(|cursor| cursor.after.as_str());
        let distance = previous.as_ref().and_then(|cursor| cursor.distance);
        let variables = Variables::new(subreddit, sort, navigation_session_id, after, distance);
        let query = PersistedQuery {
            name: "SubredditFeedSdui".into(),
            sha256_hash: "fcc9854f03d7829810b2b6f607bae27407d2bbb576b09d8ee59869f80b13ca10".into(),
        };
        let response = self.execute_graphql_post(&query, &variables).await?;
        let response = decode_response(response, FeedData::into_page);
        let not_found = response.errors().iter().any(|error| {
            error
                .pointer("/extensions/code")
                .and_then(|code| code.as_str())
                == Some("NOT_FOUND")
                && error.get("message").and_then(|message| message.as_str())
                    == Some("Resource not found")
                && error.get("path") == Some(&json!(["subredditV3", "elements"]))
        });
        let page = response.into_data().map_err(|error| match error {
            Error::OperationFailed if not_found => Error::NotFound,
            error => error,
        })?;
        let next_cursor = if let Some(end_cursor) = page.end_cursor {
            if after == Some(end_cursor.as_str()) {
                return Err(ResponseError::InvalidOperationResponse.into());
            }
            Some(
                Continuation {
                    subreddit: subreddit.into(),
                    sort,
                    navigation_session_id,
                    after: end_cursor,
                    distance: page.distance,
                }
                .encode()?,
            )
        } else {
            None
        };
        tracing::trace!(
            posts = page.posts.len(),
            has_next_page = next_cursor.is_some(),
            "Completed subreddit listing"
        );
        Ok(SubredditPage {
            posts: page.posts,
            next_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeRange;
    use crate::backends::mobile::testing::{
        self, PATH, TOKEN_FAKE, cached_storage, graphql_client,
    };
    use serde_json::Value;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-feed.json"
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn when_captured_pages_are_replayed_then_rows_and_opaque_continuation_should_match_the_sanitized_capture()
     {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/captured-subreddit-feed-pages.json"
        ))
        .unwrap();
        let server = MockServer::start().await;
        let client = graphql_client(&server, cached_storage());
        let mut previous: Option<Continuation> = None;
        let mut token = None;

        for page in fixture["pages"].as_array().unwrap() {
            server.reset().await;
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(page["body"].clone()))
                .expect(1)
                .mount(&server)
                .await;

            let result = client
                .list_subreddit_posts("rust", SubredditSort::Hot, token.as_deref())
                .await
                .unwrap();

            assert_eq!(
                serde_json::to_value(result.posts).unwrap(),
                page["expected_posts"]
            );
            let current = Continuation::decode(
                result.next_cursor.as_deref().unwrap(),
                "rust",
                SubredditSort::Hot,
            )
            .unwrap();
            let elements = &page["body"]["data"]["subredditV3"]["elements"];
            assert_eq!(
                current.after,
                elements["pageInfo"]["endCursor"].as_str().unwrap()
            );
            assert_eq!(current.distance, elements["dist"].as_i64());
            let requests = server.received_requests().await.unwrap();
            assert_eq!(requests.len(), 1);
            let request: Value = serde_json::from_slice(&requests[0].body).unwrap();
            if let Some(previous) = previous {
                assert_eq!(
                    current.navigation_session_id,
                    previous.navigation_session_id
                );
                assert_eq!(request["variables"]["after"], previous.after);
                assert_eq!(
                    request["variables"]["adContextInput"]["distance"],
                    json!(previous.distance)
                );
            }
            token = result.next_cursor;
            previous = Some(current);
            server.verify().await;
        }
    }

    #[tokio::test]
    async fn when_two_pages_are_requested_then_the_backend_should_carry_context_without_hidden_navigation_state()
     {
        let server = MockServer::start().await;
        let mut second_body = fixture();
        second_body["data"]["subredditV3"]["elements"]["dist"] = 3.into();
        second_body["data"]["subredditV3"]["elements"]["pageInfo"]["endCursor"] = Value::Null;
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", format!("Bearer {TOKEN_FAKE}")))
            .and(header("x-apollo-operation-name", "SubredditFeedSdui"))
            .and(header(
                "x-apollo-operation-id",
                "fcc9854f03d7829810b2b6f607bae27407d2bbb576b09d8ee59869f80b13ca10",
            ))
            .and(|request: &Request| {
                serde_json::from_slice::<Value>(&request.body).unwrap()["variables"]
                    .get("after")
                    .is_none()
            })
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture()))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(path("/"))
            .and(body_partial_json(json!({"variables": {"after": "upstream-page-2+/=", "adContextInput": {"distance": 4}}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(second_body)).expect(1).mount(&server).await;
        let storage = cached_storage();
        let before = storage.store().load().await.unwrap();
        let client = graphql_client(&server, storage.clone());

        let first = client
            .list_subreddit_posts("rust", SubredditSort::Hot, None)
            .await
            .unwrap();
        let second = client
            .list_subreddit_posts("rust", SubredditSort::Hot, first.next_cursor.as_deref())
            .await
            .unwrap();

        assert_eq!(first.posts.len(), 3);
        assert_eq!(second.posts, first.posts);
        assert_eq!(second.next_cursor, None);
        assert_eq!(storage.store().load().await.unwrap(), before);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        let first_request: Value = serde_json::from_slice(&requests[0].body).unwrap();
        let second_request: Value = serde_json::from_slice(&requests[1].body).unwrap();
        let navigation = Uuid::parse_str(
            first_request["variables"]["feedContextInput"]["navigationSessionId"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(navigation.get_version_num(), 4);
        assert_ne!(navigation, before.state.installation.unwrap().id);
        let mut expected: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/mobile/synthetic-subreddit-feed-request.json"
        ))
        .unwrap();
        expected["variables"]["feedContextInput"]["navigationSessionId"] = json!(navigation);
        assert_eq!(first_request, expected);
        expected["variables"]["after"] = "upstream-page-2+/=".into();
        expected["variables"]["adContextInput"]["distance"] = 4.into();
        assert_eq!(second_request, expected);
    }

    #[tokio::test]
    async fn when_a_listing_needs_authentication_then_the_backend_should_bootstrap_once_without_hydrating_rows()
     {
        let server = MockServer::start().await;
        Mock::given(path(PATH))
            .respond_with(testing::response())
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture()))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, testing::storage());

        let page = client
            .list_subreddit_posts("rust", SubredditSort::New, None)
            .await
            .unwrap();

        assert_eq!(page.posts.len(), 3);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn when_a_cursor_has_invalid_or_changed_context_then_the_backend_should_reject_it_before_bootstrap()
     {
        let server = MockServer::start().await;
        let client = graphql_client(&server, testing::storage());
        let token = Continuation {
            subreddit: "rust".into(),
            sort: SubredditSort::Hot,
            navigation_session_id: Uuid::new_v4(),
            after: "previous".into(),
            distance: None,
        }
        .encode()
        .unwrap();

        for cursor in ["invalid!", token.as_str()] {
            let result = client
                .list_subreddit_posts("rust", SubredditSort::Top(TimeRange::Day), Some(cursor))
                .await;

            assert!(matches!(
                result,
                Err(Error::InvalidRequest { field: "after" })
            ));
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn when_a_resource_is_reported_missing_then_the_backend_should_distinguish_it_from_an_unknown_query()
     {
        for (message, error_path, not_found) in [
            (
                "Resource not found",
                json!(["subredditV3", "elements"]),
                true,
            ),
            ("PersistedQueryNotFound", Value::Null, false),
            ("Resource not found", json!(["other"]), false),
        ] {
            let server = MockServer::start().await;
            let body = json!({"data": {"subredditV3": {"elements": null}}, "errors": [{"message": message, "path": error_path, "extensions": {"code": "NOT_FOUND"}}]});
            Mock::given(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .expect(1)
                .mount(&server)
                .await;
            let client = graphql_client(&server, cached_storage());

            let result = client
                .list_subreddit_posts("rust", SubredditSort::Hot, None)
                .await;

            if not_found {
                assert!(matches!(result, Err(Error::NotFound)));
            } else {
                assert!(matches!(result, Err(Error::OperationFailed)));
            }
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn when_http_denies_access_then_resource_error_text_should_not_hide_the_http_failure() {
        let server = MockServer::start().await;
        let body = json!({"errors": [{"message": "Resource not found", "path": ["subredditV3", "elements"], "extensions": {"code": "NOT_FOUND"}}]});
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(403).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let result = client
            .list_subreddit_posts("rust", SubredditSort::Hot, None)
            .await;

        assert!(matches!(result, Err(Error::AccessDenied { status: 403 })));
    }

    #[tokio::test]
    async fn when_an_unused_field_has_errors_then_the_backend_should_preserve_the_usable_page() {
        let server = MockServer::start().await;
        let mut body = fixture();
        body["errors"] = json!([{"message": "private-fixture-error", "path": ["subredditV3", "elements", "edges", 0, "node", "unused"]}]);
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let client = graphql_client(&server, cached_storage());

        let page = client
            .list_subreddit_posts("rust", SubredditSort::Hot, None)
            .await
            .unwrap();

        assert_eq!(page.posts.len(), 3);
        assert!(page.next_cursor.is_some());
        assert!(
            !serde_json::to_string(&page)
                .unwrap()
                .contains("private-fixture-error")
        );
    }

    #[tokio::test]
    async fn when_pagination_metadata_is_missing_then_the_backend_should_fail_without_advancing_state()
     {
        let server = MockServer::start().await;
        let mut body = fixture();
        body["data"]["subredditV3"]["elements"]
            .as_object_mut()
            .unwrap()
            .remove("pageInfo");
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let storage = cached_storage();
        let before = storage.store().load().await.unwrap();
        let client = graphql_client(&server, storage.clone());

        let result = client
            .list_subreddit_posts("rust", SubredditSort::Hot, None)
            .await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(
                ResponseError::InvalidOperationResponse
            ))
        ));
        assert_eq!(storage.store().load().await.unwrap(), before);
    }

    #[tokio::test]
    async fn when_the_upstream_cursor_repeats_then_the_backend_should_not_issue_a_nonadvancing_token()
     {
        let server = MockServer::start().await;
        let mut body = fixture();
        body["data"]["subredditV3"]["elements"]["pageInfo"]["endCursor"] = "same-cursor".into();
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let token = Continuation {
            subreddit: "rust".into(),
            sort: SubredditSort::Hot,
            navigation_session_id: Uuid::new_v4(),
            after: "same-cursor".into(),
            distance: Some(1),
        }
        .encode()
        .unwrap();
        let client = graphql_client(&server, cached_storage());

        let result = client
            .list_subreddit_posts("rust", SubredditSort::Hot, Some(&token))
            .await;

        assert!(matches!(
            result,
            Err(Error::InvalidResponse(
                ResponseError::InvalidOperationResponse
            ))
        ));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
