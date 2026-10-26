import type {
  CommentForest,
  CommentThread,
  Health,
  Post,
  Subreddit,
  SubredditPage,
} from "@/clients/lurklet-api/generated/types.gen";

export const HEALTH_RESPONSE = { health: "ok" } satisfies Health;

export const COMMENTS_RESPONSE = {
  appliedSort: "best",
  items: [
    {
      kind: "comment",
      comment: {
        id: "19riscw564oro",
        state: "present",
        author: { id: "htuog", name: "coolreader18" },
        createdAt: "2026-10-23T06:52:22.417Z",
        bodyMarkdown:
          "The Future and AsyncIterator proposals are very exciting. It'll be nice to eventually be able to put that class of bugs behind us; I've had to deal with them before and it's not nice.",
        score: 2,
        isOp: false,
        isPinned: false,
        initiallyCollapsed: false,
      },
      replies: [],
    },
  ],
} satisfies CommentForest;

export const COMMENT_THREAD_RESPONSE = {
  postId: "1xc54av",
  forest: {
    ...COMMENTS_RESPONSE,
    items: [...COMMENTS_RESPONSE.items, { kind: "continueThread", commentId: null }],
  },
} satisfies CommentThread;

export const SUBREDDIT_RESPONSE = {
  kind: "available",
  id: "2s7lj",
  name: "rust",
  title: "The Rust Programming Language",
  createdAt: "2010-12-02T21:27:18.336Z",
  shortDescription:
    "A place for all things related to the Rust programming language—an open-source systems language that emphasizes performance, reliability, and productivity.",
  descriptionMarkdown: `### **Please read [The Rust Community Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct)**

***

### The Rust Programming Language

A place for all things related to [the Rust programming language](https://www.rust-lang.org/)—an open-source systems language that emphasizes performance, reliability, and productivity.

***

###Rules

# Observe our code of conduct

* Strive to treat others with respect, patience, kindness, and empathy.

* We observe [the Rust Project Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

* [Details](https://reddit.com/r/rust/wiki/rules#wiki_1._observe_our_code_of_conduct)

# Submissions must be on-topic

* Posts must reference Rust or relate to things using Rust. For content that does not, use a text post to explain its relevance.

* Post titles should include useful context.

* For Rust questions, use the stickied Q&A thread.

* Arts-and-crafts posts are permitted [on weekends](https://mrmonday.github.io/craft-time/).

* No meta posts; [message the mods](https://reddit.com/message/compose/?to=/r/rust) instead.

* [Details](https://reddit.com/r/rust/wiki/rules#wiki_2._submissions_must_be_on-topic)

# Constructive criticism only

* Criticism is encouraged, though it must be constructive, useful and actionable.

* If criticizing a project on GitHub, you may not link directly to the project's issue tracker. Please [create a read-only mirror](https://archive.org/web/) and link that instead.

* [Details](https://reddit.com/r/rust/wiki/rules#wiki_3._constructive_criticism_only)

# Keep things in perspective

* A programming language is rarely worth getting worked up over.

* No zealotry or fanaticism.

* Be charitable in intent. Err on the side of giving others the benefit of the doubt.

* [Details](https://reddit.com/r/rust/wiki/rules#wiki_4._keep_things_in_perspective)

# No endless relitigation

* Avoid re-treading topics that have been long-settled or utterly exhausted.

* Avoid bikeshedding.

* This is not an official Rust forum, and cannot fulfill feature requests. Use the official venues for that.

* [Details](https://reddit.com/r/rust/wiki/rules#wiki_5._no_endless_re-litigation)

# No low-effort content

* No memes, image macros, etc.

* Consider the existing content of the subreddit and whether your post fits in. Does it inspire thoughtful discussion?

* Use properly formatted text to share code samples and error messages. Do not use images.

* Submissions appearing to contain AI-generated content may be removed at moderator discretion.

* [Details](https://reddit.com/r/rust/wiki/rules#wiki_6._no_low-effort_content)

# No code dumps

* Applications who just happen to be written in Rust are not on-topic.

* Libraries & Developer Tools for Rust developers are allowed if they follow [the No Code Dumps rules](https://www.reddit.com/mod/rust/wiki/rules#wiki_7._no_code_dumps).

* Casual reports of the joys and pains of working with Rust, or progress on Rust projects, should go in the [What's Everyone Working on this Week? thread](https://new.reddit.com/r/rust/?f=flair_name%3A%22%F0%9F%90%9D%20activity%20megathread%22).

***

### Useful Links

# Megathreads

Most links here will now take you to a search page listing posts with the relevant flair. The latest megathread for that flair should be the top result.

* [Alternative Rust Discussion Venues](https://www.reddit.com/r/rust/comments/14921t7/alternative_rust_discussion_venues/)
* [Official Blog Posts](https://www.reddit.com/r/rust/search?sort=new&restrict_sr=on&q=flair%3A%F0%9F%93%A1%2Bofficial%2Bblog)
* [Rust Foundation Posts](https://www.reddit.com/r/rust/search?sort=new&restrict_sr=on&q=flair%3A%F0%9F%AB%B1%F0%9F%8F%BB%E2%80%8D%F0%9F%AB%B2%F0%9F%8F%BE%2Bfoundation)
* [Got a Question?](https://www.reddit.com/r/rust/search?sort=new&restrict_sr=on&q=flair%3A%F0%9F%99%8B%2Bquestions%2Bmegathread)
* [What's Everyone Working On?](https://www.reddit.com/r/rust/search?sort=new&restrict_sr=on&q=flair%3A%F0%9F%90%9D%2Bactivity%2Bmegathread)
* [Who's Hiring? Jobs Threads](https://www.reddit.com/r/rust/search?sort=new&restrict_sr=on&q=flair%3A%F0%9F%92%BC%2Bjobs%2Bmegathread)
* [This Week in Rust](https://www.reddit.com/r/rust/search?sort=new&restrict_sr=on&q=flair%3A%F0%9F%93%85%2Bthis%2Bweek%2Bin%2Brust)

# Official Resources

* [Official Website](https://www.rust-lang.org)
* [Official Blog](https://blog.rust-lang.org/)
* [This Week In Rust](https://this-week-in-rust.org/)
* [Installers](https://www.rust-lang.org/install.html)
* [Source Code](https://github.com/rust-lang/)
* [Bug Tracker](https://github.com/rust-lang/rust/issues)

# Learn Rust

* [The Rust E-Book](https://doc.rust-lang.org/book/)
* [Stdlib API Reference](https://doc.rust-lang.org/std/index.html)
* [Rust By Example](https://doc.rust-lang.org/rust-by-example/index.html)
* [Rustlings](https://github.com/rust-lang/rustlings)
* [Online Playground](https://play.rust-lang.org/)

# Discussion Platforms

* [Official Users Forum](https://users.rust-lang.org/)
* [Community Discord](https://discord.gg/rust-lang-community)
* [Mozilla Matrix Chat](https://chat.mozilla.org/#/room/#rust:mozilla.org)
* [Stack Overflow Chat](https://stackoverflow.com/questions/tagged/rust)`,
  subscriberCount: 427940,
  weeklyActiveUserCount: 154424,
  weeklyContributionCount: 2925,
  styles: {
    iconUrl:
      "https://styles.redditmedia.com/t5_2s7lj/styles/communityIcon_pjg3ktzyju771.png?width=64&frame=1&auto=webp&s=5671c7524b4e747cb1cb2d48f9fda80f3ceaefce",
    legacyIconUrl:
      "https://b.thumbs.redditmedia.com/RGBpWy1J00g1sxC71l84oSYnZpvh5DOGUhcbtKz9QcE.png",
    bannerUrl: null,
    mobileBannerUrl: null,
    legacyBannerUrl: null,
    primaryColor: "#FF8D3A",
    legacyPrimaryColor: "#FF8717",
  },
  communityType: "PUBLIC",
  isNsfw: false,
  isPostingRestricted: true,
  isQuarantined: false,
  quarantineMessageMarkdown: null,
  interstitialWarningMarkdown: null,
} satisfies Subreddit;

export const POST_RESPONSE = {
  id: "1xc54av",
  title: "This Week in Rust #674",
  author: { id: "eki22", name: "seino_chan" },
  community: { id: "2s7lj", name: "rust" },
  createdAt: "2026-10-22T01:16:16.094Z",
  score: 44,
  commentCount: 2,
  isNsfw: false,
  isSpoiler: false,
  bodyMarkdown: null,
  attachments: [
    { kind: "link", url: "https://this-week-in-rust.org/blog/2026/10/21/this-week-in-rust-674/" },
  ],
  crosspostId: null,
} satisfies Post;

export const SUBREDDIT_PAGE_RESPONSE = {
  posts: [
    {
      id: "1xc54av",
      title: "This Week in Rust #674",
      thumbnail: null,
      score: 44,
      commentCount: 2,
      authorName: "seino_chan",
      createdAt: "2026-10-22T01:16:16.094Z",
      isNsfw: false,
      isSpoiler: false,
      isPinned: true,
    },
    {
      id: "1x9w6by",
      title: "Hey Rustaceans! Got a question? Ask here (43/2026)!",
      thumbnail: null,
      score: 9,
      commentCount: 6,
      authorName: "llogiq",
      createdAt: "2026-10-19T12:57:09.447Z",
      isNsfw: false,
      isSpoiler: false,
      isPinned: true,
    },
    {
      id: "1xf9qev",
      title: "This Month in Redox OS - September 2026",
      thumbnail: null,
      score: 10,
      commentCount: 1,
      authorName: "ribbon_45",
      createdAt: "2026-10-25T18:23:29.939Z",
      isNsfw: false,
      isSpoiler: false,
      isPinned: false,
    },
    {
      id: "1xeyq76",
      title: "I want more mobile apps to be developed in Rust",
      thumbnail: null,
      score: 30,
      commentCount: 41,
      authorName: "kosumi_dev",
      createdAt: "2026-10-25T10:17:21.310Z",
      isNsfw: false,
      isSpoiler: false,
      isPinned: false,
    },
    {
      id: "1xf6ib7",
      title: "Rust rules for the Please build system",
      thumbnail: null,
      score: 7,
      commentCount: 3,
      authorName: "Nearby-Blood-9817",
      createdAt: "2026-10-25T16:16:34.897Z",
      isNsfw: false,
      isSpoiler: false,
      isPinned: false,
    },
    {
      id: "1xf88gz",
      title: "I face a problem with opensource projects",
      thumbnail: null,
      score: 5,
      commentCount: 25,
      authorName: "Afraid_Interview_749",
      createdAt: "2026-10-25T17:24:10.012Z",
      isNsfw: false,
      isSpoiler: false,
      isPinned: false,
    },
  ],
  nextCursor:
    "eyJzdWJyZWRkaXQiOiJydXN0Iiwic29ydCI6ImhvdCIsIm5hdmlnYXRpb25fc2Vzc2lvbl9pZCI6IjhiZjA0NjEyLTNlYjYtNGQ4OS1hYTRkLWY2NGY3ZDBlODA1NyIsImFmdGVyIjoiZEROZk1YaG1PRGhuZWc9PSIsImRpc3RhbmNlIjo0fQ",
} satisfies SubredditPage;

export const SUBREDDIT_NEXT_PAGE_RESPONSE = {
  posts: [
    {
      ...SUBREDDIT_PAGE_RESPONSE.posts[2]!,
      id: "next123",
      title: "A second-page Rust post",
    },
  ],
  nextCursor: null,
} satisfies SubredditPage;
