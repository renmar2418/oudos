#![forbid(unsafe_code)]
//! Binary entry point for `oud-server` (control plane & signaling).

use std::net::SocketAddr;

use oud_protocol::identity::IdentityKeyPair;
use oud_server::{build_router, AppState};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db_path = std::env::var("OUD_DB_PATH").unwrap_or_else(|_| "oudos.db".to_string());
    let domain = std::env::var("OUD_SERVER_DOMAIN").unwrap_or_else(|_| "localhost".to_string());
    let bind_addr: SocketAddr = std::env::var("OUD_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()
        .expect("valid OUD_BIND_ADDR");

    let keypair = IdentityKeyPair::generate();
    let state = AppState::new(&db_path, domain, keypair).expect("initialize server state");
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("bind oud-server listener");
    tracing::info!("oud-server listening on {bind_addr}");
    axum::serve(listener, app).await.expect("serve oud-server");
}
