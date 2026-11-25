//! Integration tests for `oud-server` covering enrollment, auth, rate-limit
//! lockout, fail-closed WebSocket signaling, and verifiable hash-chained CSV export.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use futures_util::{SinkExt, StreamExt};
use oud_policy::PolicyLayer;
use oud_protocol::audit::{AuditChain, AuditEvent, AuditEventKind, GENESIS_HASH};
use oud_protocol::grant::GrantPermissions;
use oud_protocol::identity::{DeviceId, IdentityKeyPair, IdentityPublicKey, IdentitySignature};
use oud_server::audit::AppendAuditResponse;
use oud_server::auth::{ChallengeResponse, RegisterResponse, VerifyResponse};
use oud_server::rate_limit::BlockedRequesterRecord;
use oud_server::signaling::SignalEnvelope;
use oud_server::support_links::{
    AbuseReportRecord, AbuseReportResponse, CreateSupportLinkResponse, RedeemSupportLinkResponse,
};
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

async fn spawn_test_server_with_clock(
    initial_now: DateTime<Utc>,
) -> (TestServer, Arc<Mutex<DateTime<Utc>>>) {
    let keypair = IdentityKeyPair::generate();
    let server_pubkey = keypair.public_key();
    let clock_state = Arc::new(Mutex::new(initial_now));
    let clock_clone = Arc::clone(&clock_state);
    let state = AppState::with_clock(
        ":memory:",
        "support.example.org",
        keypair,
        Arc::new(move || *clock_clone.lock().expect("clock lock")),
    )
    .expect("in-memory db");
    let app = build_router(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr: SocketAddr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve test app");
    });

    (
        TestServer {
            base_url: format!("http://{addr}"),
            ws_base_url: format!("ws://{addr}"),
            server_pubkey,
        },
        clock_state,
    )
}

async fn enroll_device_for_test(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    device_id_str: &str,
) {
    let host_kp = IdentityKeyPair::generate();
    let device_id = DeviceId::parse(device_id_str).expect("valid id");
    let proof_msg = format!(
        "support.example.org:{}:{}",
        device_id.as_compact(),
        host_kp.public_key().to_prefixed_hex()
    );
    let res = client
        .post(format!("{base_url}/api/v1/devices/enroll"))
        .bearer_auth(token)
        .json(&json!({
            "device_id": device_id,
            "device_pubkey": host_kp.public_key(),
            "alias": format!("Device-{device_id_str}"),
            "os_name": "Windows 11",
            "host_signature": host_kp.sign(proof_msg.as_bytes())
        }))
        .send()
        .await
        .expect("enroll");
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn support_link_enforces_15_minute_expiry_single_use_and_server_binding() {
    let t0 = Utc::now();
    let (srv, clock) = spawn_test_server_with_clock(t0).await;
    let client = reqwest::Client::new();
    let owner_pk = IdentityKeyPair::generate();
    let tech_pk = IdentityKeyPair::generate();
    let owner_tok =
        authenticate_user(&client, &srv.base_url, "owner1@example.org", &owner_pk).await;
    let tech_tok = authenticate_user(&client, &srv.base_url, "tech1@example.org", &tech_pk).await;

    enroll_device_for_test(&client, &srv.base_url, &owner_tok, "482190734").await;

    // TTL > 900s (15 min) rejected.
    let too_long = client
        .post(format!("{}/api/v1/support-links", srv.base_url))
        .bearer_auth(&owner_tok)
        .json(&json!({ "device_id": "482190734", "ttl_secs": 901 }))
        .send()
        .await
        .expect("create too long");
    assert_eq!(too_long.status(), StatusCode::BAD_REQUEST);

    // Create fresh 15-min link and verify server binding + single use.
    let link_res = client
        .post(format!("{}/api/v1/support-links", srv.base_url))
        .bearer_auth(&owner_tok)
        .json(&json!({ "device_id": "482190734" }))
        .send()
        .await
        .expect("create link");
    assert_eq!(link_res.status(), StatusCode::OK);
    let link: CreateSupportLinkResponse = link_res.json().await.expect("link json");
    assert_eq!(link.server_domain, "support.example.org");
    assert_eq!(link.server_pubkey, srv.server_pubkey);

    // Mismatched server domain or pubkey fails closed.
    let wrong_domain = client
        .post(format!("{}/api/v1/support-links/redeem", srv.base_url))
        .bearer_auth(&tech_tok)
        .json(&json!({
            "token": link.token,
            "expected_server_domain": "evil-impostor.example"
        }))
        .send()
        .await
        .expect("wrong domain");
    assert_eq!(wrong_domain.status(), StatusCode::FORBIDDEN);

    let wrong_pk = client
        .post(format!("{}/api/v1/support-links/redeem", srv.base_url))
        .bearer_auth(&tech_tok)
        .json(&json!({
            "token": link.token,
            "expected_server_pubkey": IdentityKeyPair::generate().public_key()
        }))
        .send()
        .await
        .expect("wrong pubkey");
    assert_eq!(wrong_pk.status(), StatusCode::FORBIDDEN);

    // Valid redemption succeeds once, then second redemption is rejected (409 Conflict).
    let ok_redeem = client
        .post(format!("{}/api/v1/support-links/redeem", srv.base_url))
        .bearer_auth(&tech_tok)
        .json(&json!({
            "token": link.token,
            "expected_server_domain": "support.example.org",
            "expected_server_pubkey": srv.server_pubkey
        }))
        .send()
        .await
        .expect("redeem ok");
    assert_eq!(ok_redeem.status(), StatusCode::OK);
    let redeemed: RedeemSupportLinkResponse = ok_redeem.json().await.expect("redeem json");
    assert_eq!(redeemed.server_domain, "support.example.org");

    let replay_redeem = client
        .post(format!("{}/api/v1/support-links/redeem", srv.base_url))
        .bearer_auth(&tech_tok)
        .json(&json!({ "token": link.token }))
        .send()
        .await
        .expect("replay redeem");
    assert_eq!(replay_redeem.status(), StatusCode::CONFLICT);

    // Advance clock past 15 minutes (> 900s) -> expired link returns 410 Gone.
    let link2: CreateSupportLinkResponse = client
        .post(format!("{}/api/v1/support-links", srv.base_url))
        .bearer_auth(&owner_tok)
        .json(&json!({ "device_id": "482190734", "ttl_secs": 900 }))
        .send()
        .await
        .expect("link2")
        .json()
        .await
        .expect("link2 json");
    *clock.lock().expect("clock") = t0 + Duration::seconds(901);

    let expired_redeem = client
        .post(format!("{}/api/v1/support-links/redeem", srv.base_url))
        .bearer_auth(&tech_tok)
        .json(&json!({ "token": link2.token }))
        .send()
        .await
        .expect("expired redeem");
    assert_eq!(expired_redeem.status(), StatusCode::GONE);
}

#[tokio::test]
async fn requester_blocking_flood_rate_limit_and_abuse_reporting() {
    let srv = spawn_test_server().await;
    let client = reqwest::Client::new();
    let owner_pk = IdentityKeyPair::generate();
    let abuser_pk = IdentityKeyPair::generate();
    let owner_tok =
        authenticate_user(&client, &srv.base_url, "owner2@example.org", &owner_pk).await;
    let abuser_tok =
        authenticate_user(&client, &srv.base_url, "abuser@example.org", &abuser_pk).await;

    enroll_device_for_test(&client, &srv.base_url, &owner_tok, "987654321").await;

    let link: CreateSupportLinkResponse = client
        .post(format!("{}/api/v1/support-links", srv.base_url))
        .bearer_auth(&owner_tok)
        .json(&json!({ "device_id": "987654321" }))
        .send()
        .await
        .expect("link")
        .json()
        .await
        .expect("link json");

    // Host submits an abuse report with automatic blocking.
    let report_res = client
        .post(format!("{}/api/v1/abuse/report", srv.base_url))
        .bearer_auth(&owner_tok)
        .json(&json!({
            "device_id": "987654321",
            "requester_pubkey": abuser_pk.public_key(),
            "category": "scam_attempt",
            "summary": "Claimed to be bank fraud department",
            "also_block": true
        }))
        .send()
        .await
        .expect("report");
    assert_eq!(report_res.status(), StatusCode::OK);
    let report: AbuseReportResponse = report_res.json().await.expect("report json");
    assert!(report.blocked);
    assert_eq!(report.sla_target, "24h");

    let blocks: Vec<BlockedRequesterRecord> = client
        .get(format!("{}/api/v1/devices/987654321/blocks", srv.base_url))
        .bearer_auth(&owner_tok)
        .send()
        .await
        .expect("list blocks")
        .json()
        .await
        .expect("blocks json");
    assert_eq!(blocks.len(), 1);

    let reports: Vec<AbuseReportRecord> = client
        .get(format!("{}/api/v1/abuse/reports", srv.base_url))
        .bearer_auth(&owner_tok)
        .send()
        .await
        .expect("list reports")
        .json()
        .await
        .expect("reports json");
    assert_eq!(reports.len(), 1);

    // Blocked requester is rejected (403) on both session requests and support link redemption.
    let blocked_req = client
        .post(format!("{}/api/v1/sessions/request", srv.base_url))
        .bearer_auth(&abuser_tok)
        .json(&json!({ "device_id": "987654321" }))
        .send()
        .await
        .expect("blocked req");
    assert_eq!(blocked_req.status(), StatusCode::FORBIDDEN);

    let blocked_redeem = client
        .post(format!("{}/api/v1/support-links/redeem", srv.base_url))
        .bearer_auth(&abuser_tok)
        .json(&json!({ "token": link.token }))
        .send()
        .await
        .expect("blocked redeem");
    assert_eq!(blocked_redeem.status(), StatusCode::FORBIDDEN);

    // Per-device aggregate flood limit rejects > 25 requests in 60s across multiple requesters.
    for i in 0..3 {
        let flood_pk = IdentityKeyPair::generate();
        let flood_tok = authenticate_user(
            &client,
            &srv.base_url,
            &format!("flood{i}@example.org"),
            &flood_pk,
        )
        .await;
        for attempt in 0..10 {
            let r = client
                .post(format!("{}/api/v1/sessions/request", srv.base_url))
                .bearer_auth(&flood_tok)
                .json(&json!({ "device_id": "987654321" }))
                .send()
                .await
                .expect("flood req");
            let total_idx = i * 10 + attempt;
            if total_idx < 25 {
                assert_eq!(r.status(), StatusCode::OK);
            } else {
                assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
            }
        }
    }
}
