use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    Router,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tokio::net::TcpListener;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::{
    api::Upstream,
    db::Db,
    routes::{self, AppState},
};

pub fn state() -> AppState {
    let db = match Db::open() {
        Ok(db) => Arc::new(db),
        Err(err) => {
            tracing::error!(%err, "sqlite store unavailable, falling back to memory");

            Arc::new(Db::in_memory().expect("in-memory sqlite"))
        }
    };

    AppState {
        up: Arc::new(Upstream::new(db.clone())),
        db,
    }
}

pub fn api_router() -> Router {
    Router::new().nest("/api", routes::router(state()))
}

/// The SPA shell must never be cached: it names the hashed asset chunks, and a
/// stale copy would point at files that no longer exist after a rebuild.
async fn no_cache_html(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let response = next.run(request).await;

    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));

    if is_html {
        let mut response = response;

        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        );

        return response.into_response();
    }

    response
}

/// Shared middleware stack for both the server and the desktop app.
pub fn with_layers(router: Router) -> Router {
    router
        .layer(axum::middleware::from_fn(no_cache_html))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

/// Locate the built frontend for the plain HTTP server.
/// Order: explicit env var, repo layout next to the manifest, then the working directory.
pub fn web_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("RES_WEB_DIR") {
        let path = PathBuf::from(dir);
        if path.join("index.html").is_file() {
            return Some(path);
        }
    }
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist"),
        PathBuf::from("web/dist"),
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("web/dist")))
            .unwrap_or_default(),
    ];
    candidates
        .into_iter()
        .find(|dir| dir.join("index.html").is_file())
}

pub fn router() -> Router {
    let mut app = api_router().route("/ping", get(|| async { "pong" }));

    app = match web_dir() {
        Some(dir) => {
            tracing::info!(dir = %dir.display(), "serving built UI");
            let index = dir.join("index.html");
            app.fallback_service(ServeDir::new(&dir).not_found_service(ServeFile::new(index)))
        }
        None => {
            tracing::info!("web/dist not found - serving the UI embedded in this binary");
            app.fallback(crate::assets::embedded)
        }
    };

    with_layers(app)
}

pub async fn serve_listener(listener: TcpListener) -> std::io::Result<()> {
    axum::serve(listener, router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
        })
        .await
}

pub async fn serve(addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    serve_listener(listener).await
}
