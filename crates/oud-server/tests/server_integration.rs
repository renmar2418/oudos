//! Integration tests for `oud-server` covering enrollment, auth, rate-limit
//! lockout, fail-closed WebSocket signaling, and verifiable hash-chained CSV export.

use std::net::SocketAddr;

use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use oud_policy::PolicyLayer;
use oud_protocol::audit::{AuditChain, AuditEvent, AuditEventKind, GENESIS_HASH};
use oud_protocol::grant::GrantPermissions;
use oud_protocol::identity::{DeviceId, IdentityKeyPair, IdentityPublicKey, IdentitySignature};
use oud_server::audit::AppendAuditResponse;
use oud_server::auth::{ChallengeResponse, RegisterResponse, VerifyResponse};
use oud_server::signaling::SignalEnvelope;
use oud_server::{
    build_router, AppState, DevicePresence, DeviceRecord, EnrollDeviceResponse, ServerInfoResponse,
    SessionOutcomeResponse,
};
use reqwest::StatusCode;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

struct TestServer {
    base_url: String,
    ws_base_url: String,
    server_pubkey: IdentityPublicKey,
}

async fn spawn_test_server() -> TestServer {
    let keypair = IdentityKeyPair::generate();
    let server_pubkey = keypair.public_key();
    let state = AppState::new(":memory:", "support.example.org", keypair).expect("in-memory db");
    let app = build_router(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr: SocketAddr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve test app");
    });

    TestServer {
        base_url: format!("http://{addr}"),
        ws_base_url: format!("ws://{addr}"),
        server_pubkey,
    }
}

async fn authenticate_user(
    client: &reqwest::Client,
    base_url: &str,
    email: &str,
    passkey: &IdentityKeyPair,
) -> String {
    let reg_res = client
        .post(format!("{base_url}/api/v1/auth/register"))
        .json(&json!({
            "email": email,
            "passkey_pubkey": passkey.public_key(),
            "mfa_secret": "123456"
        }))
        .send()
        .await
        .expect("register");
    assert_eq!(reg_res.status(), StatusCode::OK);
    let _reg: RegisterResponse = reg_res.json().await.expect("register json");

    let chal_res = client
        .post(format!("{base_url}/api/v1/auth/challenge"))
        .json(&json!({ "email": email }))
        .send()
        .await
        .expect("challenge");
    assert_eq!(chal_res.status(), StatusCode::OK);
    let chal: ChallengeResponse = chal_res.json().await.expect("challenge json");

    let msg = format!("{}:{}", chal.challenge_id, chal.nonce_hex);
    let sig = passkey.sign(msg.as_bytes());

    let ver_res = client
        .post(format!("{base_url}/api/v1/auth/verify"))
        .json(&json!({
            "challenge_id": chal.challenge_id,
            "signature": sig,
            "mfa_code": "123456"
        }))
        .send()
        .await
        .expect("verify");
    assert_eq!(ver_res.status(), StatusCode::OK);
    let ver: VerifyResponse = ver_res.json().await.expect("verify json");
    ver.access_token
}

#[tokio::test]
async fn enrollment_presence_and_auth_enforcement() {
    let srv = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Unauthenticated request fails closed.
    let unauth = client
        .get(format!("{}/api/v1/devices", srv.base_url))
        .send()
        .await
        .expect("unauth devices");
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

    // Pre-paired server info returns raw domain and pubkey.
    let info: ServerInfoResponse = client
        .get(format!("{}/api/v1/server-info", srv.base_url))
        .send()
        .await
        .expect("server info")
        .json()
        .await
        .expect("server info json");
    assert_eq!(info.server_domain, "support.example.org");
    assert_eq!(info.server_pubkey, srv.server_pubkey);

    let passkey = IdentityKeyPair::generate();
    let token = authenticate_user(&client, &srv.base_url, "owner@example.org", &passkey).await;

    // Enroll device with host signature.
    let host_kp = IdentityKeyPair::generate();
    let device_id = DeviceId::parse("482190734").expect("valid device id");
    let proof_msg = format!(
        "support.example.org:{}:{}",
        device_id.as_compact(),
        host_kp.public_key().to_prefixed_hex()
    );
    let host_sig = host_kp.sign(proof_msg.as_bytes());

    let enroll_res = client
        .post(format!("{}/api/v1/devices/enroll", srv.base_url))
        .bearer_auth(&token)
        .json(&json!({
            "device_id": device_id,
            "device_pubkey": host_kp.public_key(),
            "alias": "Workstation-01",
            "os_name": "Windows 11",
            "host_signature": host_sig
        }))
        .send()
        .await
        .expect("enroll");
    assert_eq!(enroll_res.status(), StatusCode::OK);
    let enrolled: EnrollDeviceResponse = enroll_res.json().await.expect("enroll json");
    assert!(srv
        .server_pubkey
        .verify(proof_msg.as_bytes(), &enrolled.server_signature)
        .is_ok());

    // Update presence and verify device list.
    let pres_res = client
        .post(format!(
            "{}/api/v1/devices/482190734/presence",
            srv.base_url
        ))
        .bearer_auth(&token)
        .json(&json!({ "presence": "in_session" }))
        .send()
        .await
        .expect("presence");
    assert_eq!(pres_res.status(), StatusCode::OK);

    let devices: Vec<DeviceRecord> = client
        .get(format!("{}/api/v1/devices", srv.base_url))
        .bearer_auth(&token)
        .send()
        .await
        .expect("list devices")
        .json()
        .await
        .expect("devices json");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].presence, DevicePresence::InSession);
    assert_eq!(devices[0].alias, "Workstation-01");
}

#[tokio::test]
async fn rate_limit_lockout_triggers_after_three_denials() {
    let srv = spawn_test_server().await;
    let client = reqwest::Client::new();
    let passkey = IdentityKeyPair::generate();
    let token = authenticate_user(&client, &srv.base_url, "tech@example.org", &passkey).await;

    let host_kp = IdentityKeyPair::generate();
    let device_id = DeviceId::parse("123456789").expect("valid id");
    let proof_msg = format!(
        "support.example.org:{}:{}",
        device_id.as_compact(),
        host_kp.public_key().to_prefixed_hex()
    );
    client
        .post(format!("{}/api/v1/devices/enroll", srv.base_url))
        .bearer_auth(&token)
        .json(&json!({
            "device_id": device_id,
            "device_pubkey": host_kp.public_key(),
            "alias": "Target-PC",
            "os_name": "Windows 11",
            "host_signature": host_kp.sign(proof_msg.as_bytes())
        }))
        .send()
        .await
        .expect("enroll");

    // Initial request succeeds.
    let req1 = client
        .post(format!("{}/api/v1/sessions/request", srv.base_url))
        .bearer_auth(&token)
        .json(&json!({ "device_id": "123456789" }))
        .send()
        .await
        .expect("request 1");
    assert_eq!(req1.status(), StatusCode::OK);

    // Record 3 denials/mismatches.
    for strike in 1..=3 {
        let out: SessionOutcomeResponse = client
            .post(format!(
                "{}/api/v1/sessions/123456789/outcome",
                srv.base_url
            ))
            .bearer_auth(&token)
            .json(&json!({
                "requester_pubkey": passkey.public_key(),
                "outcome": if strike == 2 { "sas_mismatch" } else { "denied" }
            }))
            .send()
            .await
            .expect("outcome")
            .json()
            .await
            .expect("outcome json");
        assert_eq!(out.denial_streak, strike);
        assert_eq!(out.locked_out, strike == 3);
    }

    // Subsequent session request is locked out with 429.
    let req_locked = client
        .post(format!("{}/api/v1/sessions/request", srv.base_url))
        .bearer_auth(&token)
        .json(&json!({ "device_id": "123456789" }))
        .send()
        .await
        .expect("request locked");
    assert_eq!(req_locked.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn signaling_drop_fails_closed() {
    let srv = spawn_test_server().await;

    let (mut host_ws, _) = connect_async(format!(
        "{}/ws/signal?peer_id=device:482190734",
        srv.ws_base_url
    ))
    .await
    .expect("host ws");
    let (mut viewer_ws, _) = connect_async(format!(
        "{}/ws/signal?peer_id=viewer:user1",
        srv.ws_base_url
    ))
    .await
    .expect("viewer ws");

    let session_id = Uuid::now_v7().to_string();
    let init = serde_json::to_string(&SignalEnvelope::SessionInit {
        session_id: session_id.clone(),
        target_peer: "device:482190734".to_string(),
    })
    .expect("serialize init");
    viewer_ws
        .send(Message::Text(init))
        .await
        .expect("send init");

    // Host receives SessionInit.
    let host_msg = host_ws.next().await.expect("host msg").expect("ok");
    let Message::Text(host_text) = host_msg else {
        panic!("expected text");
    };
    let parsed: SignalEnvelope = serde_json::from_str(&host_text).expect("parse init");
    assert!(matches!(parsed, SignalEnvelope::SessionInit { .. }));

    // Host drops connection before GrantActivated.
    host_ws.close(None).await.expect("close host");

    // Viewer must fail closed with SignalingDropped.
    let viewer_msg = viewer_ws.next().await.expect("viewer msg").expect("ok");
    let Message::Text(viewer_text) = viewer_msg else {
        panic!("expected text");
    };
    let drop_env: SignalEnvelope = serde_json::from_str(&viewer_text).expect("parse drop");
    assert!(matches!(
        drop_env,
        SignalEnvelope::SignalingDropped {
            session_id: sid,
            ..
        } if sid == session_id
    ));
}

#[tokio::test]
async fn verifiable_hash_chained_audit_csv_export() {
    let srv = spawn_test_server().await;
    let client = reqwest::Client::new();
    let passkey = IdentityKeyPair::generate();
    let token = authenticate_user(&client, &srv.base_url, "auditor@example.org", &passkey).await;

    let host_kp = IdentityKeyPair::generate();
    let mut chain = AuditChain::default();
    let session_id = Uuid::now_v7();

    let e0 = chain
        .append(
            AuditEvent {
                event_id: Uuid::now_v7(),
                timestamp_utc: Utc::now(),
                session_id,
                device_pubkey: host_kp.public_key(),
                actor_pubkey: passkey.public_key(),
                server_domain: "support.example.org".to_string(),
                kind: AuditEventKind::SessionRequested,
            },
            &host_kp,
        )
        .expect("append 0")
        .clone();

    let e1 = chain
        .append(
            AuditEvent {
                event_id: Uuid::now_v7(),
                timestamp_utc: Utc::now(),
                session_id,
                device_pubkey: host_kp.public_key(),
                actor_pubkey: passkey.public_key(),
                server_domain: "support.example.org".to_string(),
                kind: AuditEventKind::GrantIssued {
                    permissions: GrantPermissions::default(),
                },
            },
            &host_kp,
        )
        .expect("append 1")
        .clone();

    for (idx, entry) in [&e0, &e1].into_iter().enumerate() {
        let res = client
            .post(format!("{}/api/v1/audit", srv.base_url))
            .json(entry)
            .send()
            .await
            .expect("post audit");
        assert_eq!(res.status(), StatusCode::OK);
        let body: AppendAuditResponse = res.json().await.expect("audit resp");
        assert_eq!(body.sequence, idx as u64);
    }

    let mut bad_chain = e1.clone();
    bad_chain.sequence = 2;
    let conflict_res = client
        .post(format!("{}/api/v1/audit", srv.base_url))
        .json(&bad_chain)
        .send()
        .await
        .expect("post conflict");
    assert_eq!(conflict_res.status(), StatusCode::CONFLICT);

    let mut bad_hash = e1.clone();
    bad_hash.sequence = 2;
    bad_hash.prev_hash = e1.entry_hash;
    bad_hash.entry_hash[0] ^= 0xff;
    let bad_res = client
        .post(format!("{}/api/v1/audit", srv.base_url))
        .json(&bad_hash)
        .send()
        .await
        .expect("post tampered hash");
    assert_eq!(bad_res.status(), StatusCode::BAD_REQUEST);

    let csv_text = client
        .get(format!("{}/api/v1/audit/export.csv", srv.base_url))
        .bearer_auth(&token)
        .send()
        .await
        .expect("export csv")
        .text()
        .await
        .expect("csv text");

    let lines: Vec<&str> = csv_text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("sequence,event_id,"));

    let mut expected_prev = hex::encode(GENESIS_HASH);
    for (i, line) in lines.iter().skip(1).enumerate() {
        let cols: Vec<&str> = line.split(',').collect();
        assert_eq!(cols[0], i.to_string());
        let expected_entry = if i == 0 { &e0 } else { &e1 };
        let row_prev = cols[cols.len() - 3];
        let row_hash = cols[cols.len() - 2];
        let row_sig = cols[cols.len() - 1];
        assert_eq!(row_prev, expected_prev);
        assert_eq!(row_hash, hex::encode(expected_entry.entry_hash));
        let sig = IdentitySignature::from_prefixed_hex(row_sig).expect("valid sig");
        assert!(host_kp
            .public_key()
            .verify(&expected_entry.entry_hash, &sig)
            .is_ok());
        expected_prev = row_hash.to_string();
    }
}

#[tokio::test]
async fn monotonic_policy_rejects_owner_relaxation_via_api() {
    let srv = spawn_test_server().await;
    let client = reqwest::Client::new();
    let passkey = IdentityKeyPair::generate();
    let token = authenticate_user(&client, &srv.base_url, "policy@example.org", &passkey).await;

    let org_layer = PolicyLayer {
        allow_mouse_keyboard: Some(false),
        ..PolicyLayer::default()
    };
    let org_res = client
        .put(format!(
            "{}/api/v1/policies/organisation/default",
            srv.base_url
        ))
        .bearer_auth(&token)
        .json(&org_layer)
        .send()
        .await
        .expect("put org policy");
    assert_eq!(org_res.status(), StatusCode::OK);

    let rogue_owner = PolicyLayer {
        allow_mouse_keyboard: Some(true),
        ..PolicyLayer::default()
    };
    let owner_res = client
        .put(format!(
            "{}/api/v1/policies/device_owner/482190734",
            srv.base_url
        ))
        .bearer_auth(&token)
        .json(&rogue_owner)
        .send()
        .await
        .expect("put rogue owner");
    assert_eq!(owner_res.status(), StatusCode::FORBIDDEN);
}
