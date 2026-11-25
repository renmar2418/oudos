//! Windows GUI entrypoint for the Oudos Tauri v2 host desktop agent.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    oud_host_desktop::run();
}
