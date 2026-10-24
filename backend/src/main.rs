use std::net::SocketAddr;

use re_search_accessory_backend::server;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,re_search_accessory_backend=debug"));
    fmt().with_env_filter(filter).compact().init();

    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8787);
    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .unwrap_or_else(|_| "127.0.0.1:8787".parse().expect("fallback address"));

    tracing::info!("re-search-accessory listening on http://{addr}");
    if let Err(err) = server::serve(addr).await {
        tracing::error!(%err, "server error");
        std::process::exit(1);
    }
}
