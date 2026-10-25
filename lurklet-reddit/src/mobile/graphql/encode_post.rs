use crate::mobile::graphql::PersistedQuery;
use serde::Serialize;

#[derive(Serialize)]
struct Request<'a, V: ?Sized> {
    #[serde(rename = "operationName")]
    operation_name: &'a str,
    variables: &'a V,
    extensions: Extensions<'a>,
}

#[derive(Serialize)]
struct Extensions<'a> {
    #[serde(rename = "persistedQuery")]
    persisted_query: Identifier<'a>,
}

#[derive(Serialize)]
struct Identifier<'a> {
    version: u8,
    #[serde(rename = "sha256Hash")]
    sha256_hash: &'a str,
}

/// Encodes the hash-only envelope without interpreting or rewriting operation variables.
pub fn encode_post<V: Serialize + ?Sized>(
    query: &PersistedQuery,
    variables: &V,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&Request {
        operation_name: &query.name,
        variables,
        extensions: Extensions {
            persisted_query: Identifier {
                version: 1,
                sha256_hash: &query.sha256_hash,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_variables_are_empty_then_encoding_should_include_an_empty_object_and_only_the_hash_envelope()
     {
        let query = PersistedQuery {
            name: "ReadFixture".into(),
            sha256_hash: "pinned-hash".into(),
        };

        let body = encode_post(&query, &json!({})).unwrap();

        assert_eq!(body, br#"{"operationName":"ReadFixture","variables":{},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"pinned-hash"}}}"#);
    }

    #[test]
    fn when_variables_have_an_order_then_encoding_should_preserve_values_without_filtering() {
        #[derive(Serialize)]
        struct Variables<'a> {
            z_flag: bool,
            count: u32,
            text: &'a str,
            ids: Vec<&'a str>,
            extra: Option<&'a str>,
        }
        let query = PersistedQuery {
            name: "ReadFixture".into(),
            sha256_hash: "pinned-hash".into(),
        };
        let variables = Variables {
            z_flag: false,
            count: 0,
            text: "",
            ids: vec!["second", "first", "second"],
            extra: None,
        };

        let body = encode_post(&query, &variables).unwrap();

        assert_eq!(body, br#"{"operationName":"ReadFixture","variables":{"z_flag":false,"count":0,"text":"","ids":["second","first","second"],"extra":null},"extensions":{"persistedQuery":{"version":1,"sha256Hash":"pinned-hash"}}}"#);
    }
}
