use axum::{
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../client/web/dist/"]
struct Assets;

pub async fn static_handler(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache = if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return (
            [
                (header::CONTENT_TYPE, mime.as_ref()),
                (header::CACHE_CONTROL, cache),
            ],
            file.data,
        )
            .into_response();
    }

    // Browser navigations ask for HTML, SPA shell
    if headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|accept| {
            !accept
                .split(',')
                .any(|part| part.trim().starts_with("text/html"))
        })
    {
        return StatusCode::NOT_FOUND.into_response();
    }

    match Assets::get("index.html") {
        Some(index) => (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            index.data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
