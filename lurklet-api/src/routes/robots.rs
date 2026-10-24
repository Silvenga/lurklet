/// Disallows crawling regardless of the static directory's contents.
pub async fn robots() -> &'static str {
    "User-agent: *\nDisallow: /\n"
}
