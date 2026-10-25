use crate::commands::CommandOutput;
use clap::{Args, ValueEnum};
use lurklet_reddit::{Error, RedditBackend, RedditClient, SubredditSort, TimeRange};
use serde_json::json;

#[derive(Args)]
pub struct ListSubredditPosts {
    /// Bare community name, for example rust.
    subreddit: String,
    #[arg(long, value_enum, default_value = "hot")]
    sort: Sort,
    /// Time window for top posts. Defaults to all; invalid with hot or new.
    #[arg(long, value_enum)]
    time: Option<Range>,
    /// Opaque token from the preceding page, with the same subreddit and sort.
    #[arg(long)]
    after: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Sort {
    Hot,
    New,
    Top,
}

#[derive(Clone, Copy, ValueEnum)]
enum Range {
    Hour,
    Day,
    Week,
    Month,
    Year,
    All,
}

impl From<Range> for TimeRange {
    fn from(value: Range) -> Self {
        match value {
            Range::Hour => Self::Hour,
            Range::Day => Self::Day,
            Range::Week => Self::Week,
            Range::Month => Self::Month,
            Range::Year => Self::Year,
            Range::All => Self::All,
        }
    }
}

impl ListSubredditPosts {
    pub async fn execute<B: RedditBackend>(
        &self,
        client: &RedditClient<B>,
    ) -> Result<CommandOutput, Error> {
        if self.sort != Sort::Top && self.time.is_some() {
            return Err(Error::InvalidRequest { field: "time" });
        }
        let sort = match self.sort {
            Sort::Hot => SubredditSort::Hot,
            Sort::New => SubredditSort::New,
            Sort::Top => SubredditSort::Top(self.time.map(TimeRange::from).unwrap_or_default()),
        };
        let page = client
            .list_subreddit_posts(&self.subreddit, sort, self.after.as_deref())
            .await?;
        Ok(CommandOutput {
            result: json!(page),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use crate::commands::Command;
    use crate::commands::testing::{FakeBackend, ListingCall};
    use clap::Parser;
    use lurklet_reddit::SubredditPage;

    #[tokio::test]
    async fn when_a_listing_is_dispatched_then_the_command_should_pass_only_the_selected_feed_inputs()
     {
        for (arguments, sort, after) in [
            (
                vec!["cli", "list-subreddit-posts", "rust"],
                SubredditSort::Hot,
                None,
            ),
            (
                vec![
                    "cli",
                    "list-subreddit-posts",
                    "rust",
                    "--sort",
                    "new",
                    "--after",
                    "opaque",
                ],
                SubredditSort::New,
                Some("opaque"),
            ),
            (
                vec!["cli", "list-subreddit-posts", "rust", "--sort", "top"],
                SubredditSort::Top(TimeRange::All),
                None,
            ),
            (
                vec![
                    "cli",
                    "list-subreddit-posts",
                    "rust",
                    "--sort",
                    "top",
                    "--time",
                    "day",
                ],
                SubredditSort::Top(TimeRange::Day),
                None,
            ),
        ] {
            let arguments = Arguments::try_parse_from(arguments).unwrap();
            let Command::ListSubredditPosts(arguments) = arguments.command else {
                panic!("expected the listing command")
            };
            let page = SubredditPage {
                posts: Vec::new(),
                next_cursor: Some("next-opaque".into()),
            };
            let client = RedditClient::from_backend(FakeBackend {
                listing: Some(ListingCall {
                    subreddit: "rust".into(),
                    sort,
                    after: after.map(String::from),
                    result: Ok(page.clone()),
                }),
                ..FakeBackend::default()
            });

            let output = arguments.execute(&client).await.unwrap();

            assert_eq!(output.result, json!(page));
            assert!(output.error.is_none());
        }
    }

    #[tokio::test]
    async fn when_a_range_is_supplied_for_hot_then_the_command_should_reject_it_without_calling_the_backend()
     {
        let arguments =
            Arguments::try_parse_from(["cli", "list-subreddit-posts", "rust", "--time", "day"])
                .unwrap();
        let Command::ListSubredditPosts(arguments) = arguments.command else {
            panic!("expected the listing command")
        };
        let client = RedditClient::from_backend(FakeBackend::default());

        let result = arguments.execute(&client).await;

        assert!(matches!(
            result,
            Err(Error::InvalidRequest { field: "time" })
        ));
    }

    #[tokio::test]
    async fn when_a_subreddit_is_not_found_then_the_command_should_preserve_the_resource_error() {
        let arguments = Arguments::try_parse_from(["cli", "list-subreddit-posts", "rust"]).unwrap();
        let Command::ListSubredditPosts(arguments) = arguments.command else {
            panic!("expected the listing command")
        };
        let client = RedditClient::from_backend(FakeBackend {
            listing: Some(ListingCall {
                subreddit: "rust".into(),
                sort: SubredditSort::Hot,
                after: None,
                result: Err(Error::NotFound),
            }),
            ..FakeBackend::default()
        });

        let result = arguments.execute(&client).await;

        assert!(matches!(result, Err(Error::NotFound)));
    }
}
