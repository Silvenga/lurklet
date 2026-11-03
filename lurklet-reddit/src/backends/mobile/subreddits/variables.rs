use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variables<'a> {
    subreddit_name: &'a str,
    include_recap_fields: bool,
    include_devvit_data: bool,
}

impl<'a> Variables<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            subreddit_name: name,
            include_recap_fields: false,
            include_devvit_data: false,
        }
    }
}
