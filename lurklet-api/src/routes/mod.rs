mod get_post;
mod health;
mod list_subreddit_posts;
mod robots;

pub use get_post::router as get_post;
pub use health::router as health;
pub use list_subreddit_posts::router as list_subreddit_posts;
pub use robots::robots;
