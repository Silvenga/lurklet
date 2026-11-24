use crate::error::ApiError;
use axum::body::{Body, to_bytes};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use lurklet_reddit::{MediaFile, MediaKind};
use m3u8_rs::Playlist;
use std::str;
use url::{Position, Url};

pub async fn response(url: &str, media: MediaFile) -> Result<Response, ApiError> {
    if media.content_range.is_some() {
        return Err(ApiError::BadGateway);
    }
    let body = to_bytes(Body::from_stream(media.body), 1024 * 1024)
        .await
        .map_err(|_| ApiError::BadGateway)?;
    let base = Url::parse(url).map_err(|_| ApiError::BadRequest)?;
    let bytes = match media.kind {
        MediaKind::Dash => dash(&base, &body)?,
        MediaKind::Hls => hls(&base, &body)?,
        MediaKind::Mp4 | MediaKind::Image | MediaKind::WebVtt => unreachable!(),
    };
    Ok(([(header::CONTENT_TYPE, media.content_type)], bytes).into_response())
}

fn proxy(base: &Url, value: &str) -> Result<String, ApiError> {
    let url = base.join(value).map_err(|_| ApiError::BadGateway)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::BadGateway);
    }
    // All child requests pass through the same upstream host checks as the manifest.
    Ok(format!("/api/media/{}", &url[Position::BeforeHost..]))
}

fn dash(base: &Url, bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    let xml = str::from_utf8(bytes).map_err(|_| ApiError::BadGateway)?;
    let doc = roxmltree::Document::parse(xml).map_err(|_| ApiError::BadGateway)?;
    if !doc.root_element().has_tag_name("MPD") {
        return Err(ApiError::BadGateway);
    }
    let mut output = String::new();
    let mut previous = 0;
    for node in doc
        .descendants()
        .filter(|node| node.has_tag_name("BaseURL"))
    {
        // Reddit's static on-demand profile puts each track's BaseURL on its representation.
        if !node
            .parent()
            .is_some_and(|node| node.has_tag_name("Representation"))
            || node.children().count() != 1
        {
            return Err(ApiError::BadGateway);
        }
        let text = node
            .first_child()
            .filter(|child| child.is_text())
            .ok_or(ApiError::BadGateway)?;
        let uri = proxy(base, text.text().ok_or(ApiError::BadGateway)?.trim())?;
        let range = text.range();
        output.push_str(&xml[previous..range.start]);
        output.push_str(
            &uri.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;"),
        );
        previous = range.end;
    }
    output.push_str(&xml[previous..]);
    Ok(output.into_bytes())
}

fn hls(base: &Url, bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    let (remaining, mut playlist) =
        m3u8_rs::parse_playlist(bytes).map_err(|_| ApiError::BadGateway)?;
    if !remaining.is_empty() {
        return Err(ApiError::BadGateway);
    }
    match &mut playlist {
        Playlist::MasterPlaylist(master) => {
            // Native HLS owns its networking. Emit only known VOD playback metadata.
            master.unknown_tags.clear();
            master.session_data.clear();
            if !master.session_key.is_empty() {
                return Err(ApiError::BadGateway);
            }
            for variant in &mut master.variants {
                variant.uri = proxy(base, &variant.uri)?;
                variant.other_attributes = None;
            }
            for track in &mut master.alternatives {
                if let Some(uri) = &mut track.uri {
                    *uri = proxy(base, uri)?;
                }
                track.other_attributes = None;
            }
        }
        Playlist::MediaPlaylist(media) => {
            if !media.end_list {
                return Err(ApiError::BadGateway);
            }
            media.unknown_tags.clear();
            for segment in &mut media.segments {
                if segment.key.is_some() {
                    return Err(ApiError::BadGateway);
                }
                segment.uri = proxy(base, &segment.uri)?;
                segment.unknown_tags.clear();
                segment.daterange = None;
                if let Some(map) = &mut segment.map {
                    map.uri = proxy(base, &map.uri)?;
                    map.other_attributes.clear();
                }
            }
        }
    }
    let mut output = Vec::new();
    playlist
        .write_to(&mut output)
        .map_err(|_| ApiError::BadGateway)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_relocating_manifests_then_track_urls_should_resolve_against_their_own_document() {
        let base = Url::parse("https://v.redd.it/asset/manifest?a=root").unwrap();
        let playlist = b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"/asset/v.mp4\",BYTERANGE=\"8@0\"\n#EXTINF:4,\n#EXT-X-BYTERANGE:20@8\nv.mp4\n#EXT-X-ENDLIST\n";
        let mpd = b"<MPD><Representation><BaseURL>https://v.redd.it/other/v.mp4?a=1&amp;v=2</BaseURL><SegmentBase indexRange=\"8-15\"/></Representation></MPD>";

        let playlist = String::from_utf8(hls(&base, playlist).unwrap()).unwrap();
        let mpd = String::from_utf8(dash(&base, mpd).unwrap()).unwrap();

        assert!(playlist.contains("URI=\"/api/media/v.redd.it/asset/v.mp4\""));
        assert!(playlist.contains("#EXT-X-BYTERANGE:20@8\n"));
        assert!(
            playlist
                .lines()
                .any(|line| line == "/api/media/v.redd.it/asset/v.mp4")
        );
        assert!(mpd.contains(
            "/api/media/v.redd.it/other/v.mp4?a=1&amp;v=2</BaseURL><SegmentBase indexRange=\"8-15\""
        ));
    }
}
