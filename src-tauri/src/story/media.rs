//! The `storymedia` protocol: the sample story's pictures, served from the
//! bytes Pit Box carries (`story_engine::sample::media`), so that Story Mode
//! plays the sample with its portraits and illustrations (SPEC-story-mode,
//! Engine file format: the sample shows how everything is made). Nothing is
//! written to disk: a copy of the pictures would have to be kept in step with
//! every version of Pit Box.
//!
//! A story of the author's has its media on disk and goes through the asset
//! protocol instead (`mediaUrl` in `src/lib/story/player/speakers.ts`).
//! `http://storymedia.localhost/sample/<path inside media/>`, each segment of
//! the path URL-encoded.

use tauri::http::{header, Request, Response, StatusCode};

/// The sample's media file a request asks for, if there is one.
fn requested(path: &str) -> Option<(&'static [u8], &'static str)> {
    let inside = crate::archive::percent_decode(path.strip_prefix("/sample/")?);
    let bytes = story_engine::sample::media(&inside)?;
    // The type follows the extension: the webview shows an image only when
    // told it is one.
    let content_type = match inside.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };
    Some((bytes, content_type))
}

pub fn serve(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let path = request.uri().path();
    match requested(path) {
        Some((bytes, content_type)) => Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .body(bytes.to_vec()),
        None => {
            log::warn!("storymedia: nothing at {path}");
            Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new())
        }
    }
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(path: &str) -> Response<Vec<u8>> {
        serve(
            &Request::builder()
                .uri(format!("http://storymedia.localhost{path}"))
                .body(Vec::new())
                .unwrap(),
        )
    }

    // The sample's pictures are served as the page asks for them, typed so
    // that the webview shows them; nothing else is (SPEC-story-mode, Engine
    // file format).
    #[test]
    fn the_sample_media_are_served_and_nothing_else() {
        let found = get("/sample/portraits/friend_happy.png");
        assert_eq!(found.status(), StatusCode::OK, "a portrait of the sample");
        assert_eq!(found.headers()[header::CONTENT_TYPE], "image/png", "typed as an image");
        assert_eq!(
            found.body().as_slice(),
            story_engine::sample::media("portraits/friend_happy.png").unwrap(),
            "the bytes Pit Box carries"
        );
        assert_eq!(
            get("/sample/portraits%2Ffriend_happy.png").status(),
            StatusCode::OK,
            "an encoded separator reads the same"
        );
        for path in [
            "/sample/story.yaml",
            "/sample/../story.yaml",
            "/portraits/friend_happy.png",
            "/sample/",
        ] {
            assert_eq!(get(path).status(), StatusCode::NOT_FOUND, "{path} is not served");
        }
    }
}
