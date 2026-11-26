//! Dev-only (`#[cfg(debug_assertions)]` + `OUDOS_DEV_HARNESS=1`) loopback trigger for
//! `docs/USABILITY_TEST_SCRIPT.md`.
//!
//! Never compiled into release builds and disabled by default even in debug builds.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::{
    dev_reset_host_impl, present_incoming_request_impl, IncomingConnectionPayload, SharedHostState,
    TrayUiHandle,
};

const HARNESS_BIND_ADDR: &str = "127.0.0.1:41420";

/// Spawns the `127.0.0.1:41420` HTTP trigger server when `OUDOS_DEV_HARNESS=1`.
pub fn spawn_loopback_server(app: AppHandle) {
    let listener = match TcpListener::bind(HARNESS_BIND_ADDR) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("oudos dev harness failed to bind {HARNESS_BIND_ADDR}: {err}");
            return;
        }
    };

    let _ = thread::Builder::new()
        .name("oudos-dev-harness".to_owned())
        .spawn(move || {
            for mut stream in listener.incoming().flatten() {
                handle_connection(&app, &mut stream);
            }
        });
}

fn handle_connection(app: &AppHandle, stream: &mut TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0_u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let raw = String::from_utf8_lossy(&buf[..n]);
    let request_line = raw.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();

    if method != "POST" {
        write_json_response(
            stream,
            405,
            "Method Not Allowed",
            r#"{"error":"only POST requests are supported"}"#,
        );
        return;
    }

    let state = app.state::<SharedHostState>();
    let tray_ui = app.state::<Mutex<TrayUiHandle>>();

    let result = match path {
        "/dev/scenario-a" => {
            let _ = dev_reset_host_impl(app, &state, &tray_ui);
            present_incoming_request_impl(
                app,
                &state,
                &tray_ui,
                IncomingConnectionPayload {
                    requester_name: "Alex Rivera".to_owned(),
                    server_domain: "support.example.org".to_owned(),
                    server_enrolled: true,
                    first_time_requester: true,
                },
                false,
            )
        }
        "/dev/scenario-b" => {
            let _ = dev_reset_host_impl(app, &state, &tray_ui);
            present_incoming_request_impl(
                app,
                &state,
                &tray_ui,
                IncomingConnectionPayload {
                    requester_name: "Bank Fraud Desk".to_owned(),
                    server_domain: "help-desk.invalid".to_owned(),
                    server_enrolled: false,
                    first_time_requester: true,
                },
                true,
            )
        }
        "/dev/reset" => dev_reset_host_impl(app, &state, &tray_ui),
        _ => {
            write_json_response(stream, 404, "Not Found", r#"{"error":"unknown dev route"}"#);
            return;
        }
    };

    match result {
        Ok(snap) => {
            let body = serde_json::to_string(&snap).unwrap_or_else(|_| "{}".to_owned());
            write_json_response(stream, 200, "OK", &body);
        }
        Err(err) => {
            let body = serde_json::json!({ "error": err }).to_string();
            write_json_response(stream, 400, "Bad Request", &body);
        }
    }
}

fn write_json_response(stream: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
