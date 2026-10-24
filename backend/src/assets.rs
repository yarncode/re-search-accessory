//! The built frontend, compiled into the binaries.
//!
//! The desktop app serves it on its ephemeral loopback port, and the plain HTTP
//! server (`re-search-accessory-server`) falls back to it when no `web/dist`
//! folder is available next to the executable.

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../web/dist"]
pub struct Assets;

pub fn asset(path: &str) -> Response {
    let clean = path.trim_start_matches('/');
    let clean = if clean.is_empty() { "index.html" } else { clean };

    let (name, data) = match Assets::get(clean) {
        Some(file) => (clean, file.data.into_owned()),
        // SPA fallback: unknown paths render the app shell.
        None => match Assets::get("index.html") {
            Some(file) => ("index.html", file.data.into_owned()),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    "UI chưa được build vào app (chạy npm run build trong web/).",
                )
                    .into_response();
            }
        },
    };

    let mime = mime_guess::from_path(name).first_or_octet_stream();

    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_str(mime.as_ref())
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
        )],
        Body::from(data),
    )
        .into_response()
}

pub async fn embedded(uri: Uri) -> Response {
    asset(uri.path())
}
