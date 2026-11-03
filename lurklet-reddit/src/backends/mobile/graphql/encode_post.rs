use crate::backends::mobile::graphql::PersistedQuery;
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
