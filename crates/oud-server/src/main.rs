#![forbid(unsafe_code)]
//! Binary entry point for `oud-server` (control plane & signaling).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use oud_protocol::identity::IdentityKeyPair;
use oud_server::{build_router, load_or_create_server_keypair, AppState};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db_path = std::env::var("OUD_DB_PATH").unwrap_or_else(|_| "oudos.db".to_string());
    if let Some(parent) = Path::new(&db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).expect("create database directory");
        }
    }

    let domain = std::env::var("OUD_SERVER_DOMAIN").unwrap_or_else(|_| "localhost".to_string());
    let bind_addr: SocketAddr = std::env::var("OUD_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()
        .expect("valid OUD_BIND_ADDR");

    let keypair = resolve_server_keypair(&db_path);
    let server_pubkey = keypair.public_key();
    let state = AppState::new(&db_path, &domain, keypair).expect("initialize server state");
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("bind oud-server listener");
    tracing::info!(
        domain = %domain,
        server_pubkey = %server_pubkey.to_prefixed_hex(),
        "oud-server listening on {bind_addr}"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve oud-server");
}

fn resolve_server_keypair(db_path: &str) -> IdentityKeyPair {
    if let Ok(seed_hex) = std::env::var("OUD_SERVER_SEED_HEX") {
        let trimmed = seed_hex.trim();
        if !trimmed.is_empty() {
            let decoded = hex::decode(trimmed).expect("OUD_SERVER_SEED_HEX must be valid hex");
            let seed: [u8; 32] = decoded
                .try_into()
                .expect("OUD_SERVER_SEED_HEX must be 64 hex chars (32 bytes)");
            return IdentityKeyPair::from_seed(seed);
        }
    }

    let key_path = std::env::var("OUD_SERVER_KEY_PATH").map_or_else(
        |_| {
            let db_dir = Path::new(db_path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
            db_dir.join("server.key")
        },
        PathBuf::from,
    );
    load_or_create_server_keypair(&key_path).expect("load or create persistent server keypair")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut sig) = signal(SignalKind::terminate()) {
            let _ = sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("oud-server shutting down cleanly");
}
