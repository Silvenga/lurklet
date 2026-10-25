use crate::{PostAttachment, VideoFormat, VideoSource};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachments {
    is_self_post: Option<bool>,
    url: Option<String>,
    thumbnail: Option<MediaSource>,
    media: Option<Media>,
    gallery: Option<Gallery>,
}

#[derive(Deserialize)]
struct MediaSource {
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Media {
    type_hint: Option<String>,
    still: Option<StillMedia>,
    animated: Option<AnimatedMedia>,
    streaming: Option<StreamingMedia>,
    packaged_media: Option<PackagedMedia>,
    video: Option<MediaSource>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StillMedia {
    #[serde(alias = "content")]
    source: Option<MediaSource>,
    alt_text: Option<String>,
}

#[derive(Deserialize)]
struct AnimatedMedia {
    mp4_source: Option<MediaSource>,
    mp4_small: Option<MediaSource>,
    mp4_medium: Option<MediaSource>,
    mp4_large: Option<MediaSource>,
    mp4_xlarge: Option<MediaSource>,
    mp4_xxlarge: Option<MediaSource>,
    mp4_xxxlarge: Option<MediaSource>,
    gif_source: Option<MediaSource>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamingMedia {
    hls_url: Option<String>,
    dash_url: Option<String>,
}

impl StreamingMedia {
    fn into_sources(self) -> impl Iterator<Item = VideoSource> {
        [
            (self.hls_url, VideoFormat::Hls),
            (self.dash_url, VideoFormat::Dash),
        ]
        .into_iter()
        .filter_map(|(url, format)| url.map(|url| VideoSource { url, format }))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackagedMedia {
    muxed_mp4s: Option<MuxedMp4s>,
    playback_mp4s: Option<PlaybackMp4s>,
}

#[derive(Deserialize)]
struct MuxedMp4s {
    recommended: Option<MediaSource>,
    medium: Option<MediaSource>,
}

#[derive(Deserialize)]
struct PlaybackMp4s {
    permutations: Option<Vec<Permutation>>,
}

#[derive(Deserialize)]
struct Permutation {
    source: Option<MediaSource>,
}

impl PackagedMedia {
    fn into_sources(self) -> impl Iterator<Item = VideoSource> {
        let muxed = self
            .muxed_mp4s
            .into_iter()
            .flat_map(|media| [media.recommended, media.medium])
            .flatten();
        let playback = self
            .playback_mp4s
            .into_iter()
            .flat_map(|media| media.permutations.into_iter().flatten())
            .filter_map(|permutation| permutation.source);
        muxed.chain(playback).filter_map(|source| {
            source.url.map(|url| VideoSource {
                url,
                format: VideoFormat::Mp4,
            })
        })
    }
}

#[derive(Deserialize)]
struct Gallery {
    items: Option<Vec<Option<GalleryItem>>>,
}

#[derive(Deserialize)]
struct GalleryItem {
    caption: Option<String>,
    media: Option<GalleryMedia>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum GalleryMedia {
    #[serde(rename = "ImageAsset", alias = "AnimatedImageAsset")]
    Image(ImageAsset),
    #[serde(rename = "VideoAsset")]
    Video(Box<VideoAsset>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageAsset {
    status: Option<String>,
    url: Option<String>,
    mp4_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoAsset {
    #[serde(flatten)]
    streaming: StreamingMedia,
    packaged_media: Option<PackagedMedia>,
    still: Option<StillMedia>,
}

impl From<GalleryItem> for Option<PostAttachment> {
    fn from(item: GalleryItem) -> Self {
        match item.media? {
            GalleryMedia::Image(image) if image.status.as_deref() == Some("VALID") => {
                if let Some(url) = image.mp4_url {
                    Some(PostAttachment::Video {
                        sources: vec![VideoSource {
                            url,
                            format: VideoFormat::Mp4,
                        }],
                        poster_url: image.url,
                    })
                } else {
                    Some(PostAttachment::Image {
                        url: image.url?,
                        alt_text: None,
                        caption: item.caption,
                    })
                }
            }
            GalleryMedia::Video(video) => {
                let sources: Vec<_> = video
                    .packaged_media
                    .into_iter()
                    .flat_map(PackagedMedia::into_sources)
                    .chain(video.streaming.into_sources())
                    .collect();
                if sources.is_empty() {
                    None
                } else {
                    Some(PostAttachment::Video {
                        sources,
                        poster_url: video
                            .still
                            .and_then(|still| still.source)
                            .and_then(|source| source.url),
                    })
                }
            }
            GalleryMedia::Image(_) | GalleryMedia::Other => None,
        }
    }
}

impl Attachments {
    pub fn into_attachments(self, is_crosspost: bool) -> Vec<PostAttachment> {
        if let Some(gallery) = self.gallery {
            return gallery
                .items
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(Option::<PostAttachment>::from)
                .collect();
        }
        if let Some(media) = self.media {
            if media.type_hint.as_deref() == Some("EMBED") {
                if let Some(url) = media.video.and_then(|video| video.url) {
                    return vec![PostAttachment::Link { url }];
                }
            } else {
                let mut sources: Vec<_> = media
                    .packaged_media
                    .into_iter()
                    .flat_map(PackagedMedia::into_sources)
                    .collect();
                let mut gif = None;
                if let Some(animated) = media.animated {
                    gif = animated.gif_source;
                    sources.extend(
                        [
                            animated.mp4_source,
                            animated.mp4_small,
                            animated.mp4_medium,
                            animated.mp4_large,
                            animated.mp4_xlarge,
                            animated.mp4_xxlarge,
                            animated.mp4_xxxlarge,
                        ]
                        .into_iter()
                        .flatten()
                        .filter_map(|source| {
                            source.url.map(|url| VideoSource {
                                url,
                                format: VideoFormat::Mp4,
                            })
                        }),
                    );
                }
                sources.extend(
                    media
                        .streaming
                        .into_iter()
                        .flat_map(StreamingMedia::into_sources),
                );
                if !sources.is_empty() {
                    return vec![PostAttachment::Video {
                        sources,
                        poster_url: media
                            .still
                            .and_then(|still| still.source)
                            .and_then(|source| source.url)
                            .or_else(|| self.thumbnail.and_then(|source| source.url)),
                    }];
                }
                let (image, alt_text) = match media.type_hint.as_deref() {
                    Some("IMAGE") => media
                        .still
                        .map(|still| (still.source, still.alt_text))
                        .unwrap_or_default(),
                    Some("GIFVIDEO") => (gif, media.still.and_then(|still| still.alt_text)),
                    _ => (None, None),
                };
                if let Some(url) = image.and_then(|image| image.url) {
                    return vec![PostAttachment::Image {
                        url,
                        alt_text,
                        caption: None,
                    }];
                }
            }
        }
        if self.is_self_post == Some(false)
            && !is_crosspost
            && let Some(url) = self.url
        {
            return vec![PostAttachment::Link { url }];
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn attachments(post: &Value) -> Vec<PostAttachment> {
        Attachments::deserialize(post)
            .unwrap()
            .into_attachments(false)
    }

    #[test]
    fn when_animated_media_has_multiple_mp4s_then_projection_should_keep_every_returned_resolution()
    {
        let names = [
            "mp4_source",
            "mp4_small",
            "mp4_medium",
            "mp4_large",
            "mp4_xlarge",
            "mp4_xxlarge",
            "mp4_xxxlarge",
        ];
        let mut post = json!({"media": {"typeHint": "GIFVIDEO", "animated": {}}});
        let expected = names
            .iter()
            .map(|name| {
                let url = format!("https://example.invalid/{name}.mp4");
                post["media"]["animated"][name] = json!({"url": url});
                VideoSource {
                    url,
                    format: VideoFormat::Mp4,
                }
            })
            .collect();

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: expected,
                poster_url: None
            }]
        );
    }

    #[test]
    fn when_playback_permutations_are_returned_then_projection_should_preserve_all_sources_without_fetching_them()
     {
        let post = json!({"media": {"typeHint": "VIDEO", "packagedMedia": {"playbackMp4s": {"permutations": [
            {"source": {"url": "https://example.invalid/avc.mp4?signature=fixture", "videoCodec": "avc"}},
            {"source": {"url": "https://example.invalid/hevc.mp4", "videoCodec": "hevc"}}
        ]}}}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: vec![
                    VideoSource {
                        url: "https://example.invalid/avc.mp4?signature=fixture".into(),
                        format: VideoFormat::Mp4
                    },
                    VideoSource {
                        url: "https://example.invalid/hevc.mp4".into(),
                        format: VideoFormat::Mp4
                    },
                ],
                poster_url: None,
            }]
        );
    }

    #[test]
    fn when_gallery_video_has_packaged_and_streaming_sources_then_projection_should_keep_both_families()
     {
        let post = json!({"gallery": {"items": [{"media": {
            "__typename": "VideoAsset", "hlsUrl": "https://example.invalid/video.m3u8",
            "packagedMedia": {"muxedMp4s": {"medium": {"url": "https://example.invalid/video.mp4"}}},
            "still": {"content": {"url": "https://example.invalid/poster.jpg"}}
        }}]}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: vec![
                    VideoSource {
                        url: "https://example.invalid/video.mp4".into(),
                        format: VideoFormat::Mp4
                    },
                    VideoSource {
                        url: "https://example.invalid/video.m3u8".into(),
                        format: VideoFormat::Hls
                    },
                ],
                poster_url: Some("https://example.invalid/poster.jpg".into()),
            }]
        );
    }

    #[test]
    fn when_an_animated_gallery_asset_includes_mp4_then_projection_should_expose_the_returned_video()
     {
        let post = json!({"gallery": {"items": [{"media": {
            "__typename": "AnimatedImageAsset", "status": "VALID", "url": "https://example.invalid/animation.gif",
            "mp4Url": "https://example.invalid/animation.mp4"
        }}]}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: vec![VideoSource {
                    url: "https://example.invalid/animation.mp4".into(),
                    format: VideoFormat::Mp4
                }],
                poster_url: Some("https://example.invalid/animation.gif".into()),
            }]
        );
    }

    #[test]
    fn when_media_urls_are_null_then_serde_should_preserve_other_available_sources() {
        let post = json!({"media": {"typeHint": "VIDEO", "streaming": {"hlsUrl": null, "dashUrl": "https://example.invalid/video.mpd"},
            "packagedMedia": {"muxedMp4s": {"recommended": {"url": null}}}}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: vec![VideoSource {
                    url: "https://example.invalid/video.mpd".into(),
                    format: VideoFormat::Dash
                }],
                poster_url: None,
            }]
        );
    }

    #[test]
    fn when_consumed_media_fields_have_incorrect_types_then_serde_should_report_the_malformed_data()
    {
        let post = json!({"media": {"streaming": {"hlsUrl": 123}}});

        let result = Attachments::deserialize(&post);

        assert!(result.is_err());
    }

    #[test]
    fn when_an_external_link_has_a_thumbnail_then_projection_should_return_the_destination_not_the_thumbnail()
     {
        let post = json!({"isSelfPost": false, "url": "https://example.invalid/article", "thumbnail": {"url": "https://example.invalid/preview.jpg"}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Link {
                url: "https://example.invalid/article".into()
            }]
        );
    }

    #[test]
    fn when_a_text_post_has_a_permalink_then_projection_should_not_add_it_as_an_attachment() {
        let post = json!({"isSelfPost": true, "url": "https://www.reddit.com/r/example/comments/18t5968/"});

        let result = attachments(&post);

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn when_an_image_has_alt_text_then_projection_should_keep_one_source_and_its_description() {
        let post = json!({"media": {"typeHint": "IMAGE", "still": {"source": {"url": "https://example.invalid/image.jpg?signature=fixture&width=1200"}, "altText": "A description", "small": null}, "obfuscated_still": null}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Image {
                url: "https://example.invalid/image.jpg?signature=fixture&width=1200".into(),
                alt_text: Some("A description".into()),
                caption: None
            }]
        );
    }

    #[test]
    fn when_a_gallery_has_no_outer_media_then_projection_should_preserve_image_order_and_captions()
    {
        let post = json!({"media": null, "gallery": {"items": [
            {"id": "200", "caption": "First", "media": {"__typename": "ImageAsset", "id": "image-one", "status": "VALID", "url": "https://example.invalid/one.jpg", "xxxlarge": null}},
            {"id": "100", "caption": null, "media": {"__typename": "AnimatedImageAsset", "id": "image-two", "status": "VALID", "url": "https://example.invalid/two.gif"}}
        ]}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![
                PostAttachment::Image {
                    url: "https://example.invalid/one.jpg".into(),
                    alt_text: None,
                    caption: Some("First".into())
                },
                PostAttachment::Image {
                    url: "https://example.invalid/two.gif".into(),
                    alt_text: None,
                    caption: None
                },
            ]
        );
    }

    #[test]
    fn when_gallery_assets_are_not_ready_then_projection_should_not_advertise_them_as_displayable_images()
     {
        let post = json!({"gallery": {"items": [
            {"media": {"__typename": "ImageAsset", "status": "PROCESSING", "url": "https://example.invalid/pending.jpg"}},
            {"media": {"__typename": "ImageAsset", "status": "FAILED", "url": "https://example.invalid/failed.jpg"}}
        ]}});

        let result = attachments(&post);

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn when_video_has_muxed_and_streaming_sources_then_projection_should_return_every_source() {
        let post = json!({"media": {"typeHint": "VIDEO", "packagedMedia": {"muxedMp4s": {"recommended": {"url": "https://example.invalid/video.mp4?token=fixture"}, "medium": {"url": "https://example.invalid/medium.mp4"}}}, "streaming": {"hlsUrl": "https://example.invalid/video.m3u8", "dashUrl": "https://example.invalid/video.mpd"}, "still": {"source": {"url": "https://example.invalid/poster.jpg"}}}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: vec![
                    VideoSource {
                        url: "https://example.invalid/video.mp4?token=fixture".into(),
                        format: VideoFormat::Mp4
                    },
                    VideoSource {
                        url: "https://example.invalid/medium.mp4".into(),
                        format: VideoFormat::Mp4
                    },
                    VideoSource {
                        url: "https://example.invalid/video.m3u8".into(),
                        format: VideoFormat::Hls
                    },
                    VideoSource {
                        url: "https://example.invalid/video.mpd".into(),
                        format: VideoFormat::Dash
                    },
                ],
                poster_url: Some("https://example.invalid/poster.jpg".into())
            }]
        );
    }

    #[test]
    fn when_only_streaming_video_is_available_then_projection_should_label_its_actual_format() {
        for (field, format) in [("hlsUrl", VideoFormat::Hls), ("dashUrl", VideoFormat::Dash)] {
            let post = json!({"media": {"typeHint": "VIDEO", "streaming": {field: "https://example.invalid/manifest"}}, "thumbnail": {"url": "https://example.invalid/poster.jpg"}});

            let result = attachments(&post);

            assert_eq!(
                result,
                vec![PostAttachment::Video {
                    sources: vec![VideoSource {
                        url: "https://example.invalid/manifest".into(),
                        format
                    }],
                    poster_url: Some("https://example.invalid/poster.jpg".into())
                }]
            );
        }
    }

    #[test]
    fn when_a_gallery_contains_a_video_then_projection_should_keep_all_its_stream_urls() {
        let post = json!({"gallery": {"items": [{"media": {"__typename": "VideoAsset", "hlsUrl": "https://example.invalid/video.m3u8", "dashUrl": "https://example.invalid/video.mpd"}}]}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: vec![
                    VideoSource {
                        url: "https://example.invalid/video.m3u8".into(),
                        format: VideoFormat::Hls
                    },
                    VideoSource {
                        url: "https://example.invalid/video.mpd".into(),
                        format: VideoFormat::Dash
                    },
                ],
                poster_url: None
            }]
        );
    }

    #[test]
    fn when_an_animated_post_has_an_mp4_source_then_projection_should_not_return_its_still_preview()
    {
        let post = json!({"media": {"typeHint": "GIFVIDEO", "animated": {"mp4_source": {"url": "https://example.invalid/animation.mp4"}, "gif_source": {"url": "https://example.invalid/animation.gif"}}, "still": {"source": {"url": "https://example.invalid/poster.jpg"}}}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Video {
                sources: vec![VideoSource {
                    url: "https://example.invalid/animation.mp4".into(),
                    format: VideoFormat::Mp4
                }],
                poster_url: Some("https://example.invalid/poster.jpg".into())
            }]
        );
    }

    #[test]
    fn when_only_a_gif_source_is_available_then_projection_should_keep_the_actual_image_url() {
        let post = json!({"media": {"typeHint": "GIFVIDEO", "animated": {"gif_source": {"url": "https://example.invalid/animation.gif"}}}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Image {
                url: "https://example.invalid/animation.gif".into(),
                alt_text: None,
                caption: None
            }]
        );
    }

    #[test]
    fn when_a_video_is_an_external_embed_then_projection_should_expose_a_link_instead_of_markup() {
        let post = json!({"media": {"typeHint": "EMBED", "video": {"url": "https://example.invalid/watch", "embedHtml": "<iframe src='https://example.invalid/embed'></iframe>"}, "still": {"source": {"url": "https://example.invalid/poster.jpg"}}}});

        let result = attachments(&post);

        assert_eq!(
            result,
            vec![PostAttachment::Link {
                url: "https://example.invalid/watch".into()
            }]
        );
        assert!(!serde_json::to_string(&result).unwrap().contains("iframe"));
    }
}
