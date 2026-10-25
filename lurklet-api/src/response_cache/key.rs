use axum::http::Uri;
use form_urlencoded::{Serializer, parse};

/// Normalized URL identity and its filesystem-safe BLAKE3 digest.
pub struct CacheKey {
    uri: Uri,
    digest: String,
}

impl CacheKey {
    /// Derives identity from the path and normalized query, excluding the authority.
    pub fn new(uri: &Uri) -> Self {
        let mut pairs = parse(uri.query().unwrap_or_default().as_bytes()).collect::<Vec<_>>();
        pairs.sort_by(|left, right| left.0.cmp(&right.0));
        let query = Serializer::new(String::new()).extend_pairs(pairs).finish();
        let normalized: Uri = if query.is_empty() {
            uri.path().parse().unwrap()
        } else {
            format!("{}?{query}", uri.path()).parse().unwrap()
        };
        let digest = blake3::hash(normalized.path_and_query().unwrap().as_str().as_bytes())
            .to_hex()
            .to_string();
        Self {
            uri: normalized,
            digest,
        }
    }

    /// Returns the normalized path and query without modifying the request URI.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Returns the digest used for storage lookup and refresh coordination.
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_queries_are_equivalent_then_they_should_share_a_key() {
        let first = CacheKey::new(&"/api/posts?sort=new&q=a%20b".parse().unwrap());

        for uri in [
            "/api/posts?q=a+b&sort=new",
            "/api/posts?sort=%6Eew&q=a+b",
            "http://another-host/api/posts?q=a+b&sort=new",
        ] {
            let other = CacheKey::new(&uri.parse().unwrap());

            assert_eq!(first.uri(), other.uri());
            assert_eq!(first.digest, other.digest);
        }
        assert_eq!(first.digest.len(), 64);
        assert!(first.digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn when_values_repeat_then_normalization_should_preserve_their_order() {
        let uri = "/api/posts?z=1&tag=b&tag=a&empty&z=2".parse().unwrap();

        let key = CacheKey::new(&uri);

        assert_eq!(key.uri(), "/api/posts?empty=&tag=b&tag=a&z=1&z=2");
        assert_eq!(uri, "/api/posts?z=1&tag=b&tag=a&empty&z=2");
        let repeated = CacheKey::new(key.uri());
        assert_eq!(repeated.digest(), key.digest());
    }

    #[test]
    fn when_queries_are_empty_then_they_should_share_a_key() {
        let first = CacheKey::new(&"/api/posts".parse().unwrap());
        let second = CacheKey::new(&"/api/posts?".parse().unwrap());

        assert_eq!(first.uri(), "/api/posts");
        assert_eq!(first.digest, second.digest);
    }

    #[test]
    fn when_paths_or_values_differ_then_they_should_not_share_a_key() {
        let first = CacheKey::new(&"/api/posts?sort=new".parse().unwrap());

        for uri in [
            "/api/users?sort=new",
            "/api/posts?sort=top",
            "/api/posts?sort=new&_=1",
        ] {
            let other = CacheKey::new(&uri.parse().unwrap());

            assert_ne!(first.digest, other.digest);
        }
    }
}
