#![forbid(unsafe_code)]
//! Binary entry point for `oud-server` (control plane & signaling).

fn main() {
    let (version, _) = oud_server::server_protocol_banner();
    println!("oud-server scaffold ready ({version})");
}
