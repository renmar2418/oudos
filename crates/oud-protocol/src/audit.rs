//! Tamper-evident, metadata-only SHA-256 hash-chained audit log signed by the
//! host Ed25519 identity key.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::ProtocolError;
use crate::grant::GrantPermissions;
use crate::identity::{IdentityKeyPair, IdentityPublicKey, IdentitySignature};
use crate::state_machine::{BlockedReason, DenialReason, RevocationReason};

/// Genesis hash (`32` zero bytes) preceding entry `0` of an audit chain.
pub const GENESIS_HASH: [u8; 32] = [0u8; 32];

/// Metadata-only audit event variants.
///
/// Screen pixels, keystrokes, clipboard text, and file payloads are **never** logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditEventKind {
    /// Incoming connection request received.
    SessionRequested,
    /// Connection halted on security alert.
    BlockedSecurityAlert {
        /// Blocking reason.
        reason: BlockedReason,
    },
    /// Request denied, expired, or SAS mismatched.
    ConsentDenied {
        /// Denial reason.
        reason: DenialReason,
    },
    /// Host signed a `SessionGrant` and opened the session.
    GrantIssued {
        /// Granted permissions.
        permissions: GrantPermissions,
    },
    /// Host modified permissions mid-session.
    PermissionsUpdated {
        /// Updated permissions.
        permissions: GrantPermissions,
    },
    /// Metadata for a file transfer (never file content).
    FileTransferMetadata {
        /// File name only.
        filename: String,
        /// Payload size in bytes.
        size_bytes: u64,
    },
    /// Metadata that a clipboard sync occurred (never clipboard text).
    ClipboardSyncEvent,
    /// Active session ended or revoked.
    SessionRevoked {
        /// Revocation reason.
        reason: RevocationReason,
        /// Elapsed session duration in seconds.
        duration_secs: u64,
    },
}

/// Metadata-only audit event record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event identifier (`UUIDv7`).
    pub event_id: Uuid,
    /// UTC timestamp.
    pub timestamp_utc: DateTime<Utc>,
    /// Session identifier associated with this event.
    pub session_id: Uuid,
    /// Host Ed25519 public key.
    pub device_pubkey: IdentityPublicKey,
    /// Requester/viewer Ed25519 public key.
    pub actor_pubkey: IdentityPublicKey,
    /// Raw pinned server domain.
    pub server_domain: String,
    /// Event metadata payload.
    pub kind: AuditEventKind,
}

/// Single entry in the tamper-evident hash chain:
/// `H_n = SHA-256(H_{n-1} || CanonicalJSON(Event_n))`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedAuditEntry {
    /// Zero-based monotonic sequence index.
    pub sequence: u64,
    /// Hash of the previous entry (`GENESIS_HASH` for `sequence == 0`).
    pub prev_hash: [u8; 32],
    /// Hash of `prev_hash || CanonicalJSON(event)`.
    pub entry_hash: [u8; 32],
    /// Metadata-only event payload.
    pub event: AuditEvent,
    /// Host Ed25519 signature over `entry_hash`.
    pub host_signature: IdentitySignature,
}

/// Append-only local audit chain maintained on the host.
#[derive(Debug, Clone, Default)]
pub struct AuditChain {
    entries: Vec<SignedAuditEntry>,
}

impl AuditChain {
    /// Creates an empty audit chain.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Returns all entries in sequence order.
    #[must_use]
    pub fn entries(&self) -> &[SignedAuditEntry] {
        &self.entries
    }

    /// Appends a new metadata-only [`AuditEvent`], chains its hash, and signs it
    /// with the host Ed25519 identity key.
    ///
    /// # Errors
    /// Returns [`ProtocolError::HostKeyChanged`] if `signer` does not match `event.device_pubkey`,
    /// or [`ProtocolError::SerializationError`] if canonical JSON serialization fails.
    pub fn append(
        &mut self,
        event: AuditEvent,
        signer: &IdentityKeyPair,
    ) -> Result<&SignedAuditEntry, ProtocolError> {
        if signer.public_key() != event.device_pubkey {
            return Err(ProtocolError::HostKeyChanged);
        }

        let sequence = self.entries.len() as u64;
        let prev_hash = self
            .entries
            .last()
            .map_or(GENESIS_HASH, |last| last.entry_hash);

        let entry_hash = compute_entry_hash(&prev_hash, &event)?;
        let host_signature = signer.sign(&entry_hash);

        self.entries.push(SignedAuditEntry {
            sequence,
            prev_hash,
            entry_hash,
            event,
            host_signature,
        });

        self.entries.last().ok_or(ProtocolError::AuditChainBroken {
            index: 0,
            reason: "internal append failure",
        })
    }

    /// Verifies the full chain against `host_pubkey`.
    ///
    /// # Errors
    /// Returns [`ProtocolError::AuditChainBroken`] on any tamper, reorder, deletion,
    /// or signature failure.
    pub fn verify(&self, host_pubkey: &IdentityPublicKey) -> Result<(), ProtocolError> {
        verify_chain(&self.entries, host_pubkey)
    }
}

/// Verifies an arbitrary slice of [`SignedAuditEntry`] items for hash-chain continuity,
/// monotonic sequence numbers, and valid host Ed25519 signatures.
///
/// # Errors
/// Returns [`ProtocolError::AuditChainBroken`] on the first invalid entry.
pub fn verify_chain(
    entries: &[SignedAuditEntry],
    host_pubkey: &IdentityPublicKey,
) -> Result<(), ProtocolError> {
    let mut expected_prev = GENESIS_HASH;
    for (idx, entry) in entries.iter().enumerate() {
        if entry.sequence != idx as u64 {
            return Err(ProtocolError::AuditChainBroken {
                index: idx,
                reason: "sequence gap or reorder detected",
            });
        }
        if entry.prev_hash != expected_prev {
            return Err(ProtocolError::AuditChainBroken {
                index: idx,
                reason: "prev_hash mismatch",
            });
        }
        if &entry.event.device_pubkey != host_pubkey {
            return Err(ProtocolError::AuditChainBroken {
                index: idx,
                reason: "device_pubkey mismatch",
            });
        }
        let recomputed = compute_entry_hash(&entry.prev_hash, &entry.event)?;
        if recomputed != entry.entry_hash {
            return Err(ProtocolError::AuditChainBroken {
                index: idx,
                reason: "entry_hash mismatch",
            });
        }
        if host_pubkey
            .verify(&entry.entry_hash, &entry.host_signature)
            .is_err()
        {
            return Err(ProtocolError::AuditChainBroken {
                index: idx,
                reason: "host Ed25519 signature invalid",
            });
        }
        expected_prev = entry.entry_hash;
    }
    Ok(())
}

/// Computes `SHA-256(prev_hash || CanonicalJSON(event))`.
///
/// # Errors
/// Returns [`ProtocolError::SerializationError`] if `event` cannot be serialized.
pub fn compute_entry_hash(
    prev_hash: &[u8; 32],
    event: &AuditEvent,
) -> Result<[u8; 32], ProtocolError> {
    let canonical_json =
        serde_json::to_vec(event).map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(prev_hash);
    hasher.update(&canonical_json);
    Ok(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_chain_detects_tampering_and_reordering() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let session_id = Uuid::now_v7();
        let mut chain = AuditChain::new();

        chain
            .append(
                AuditEvent {
                    event_id: Uuid::now_v7(),
                    timestamp_utc: Utc::now(),
                    session_id,
                    device_pubkey: host.public_key(),
                    actor_pubkey: viewer,
                    server_domain: "support.example.org".to_owned(),
                    kind: AuditEventKind::SessionRequested,
                },
                &host,
            )
            .expect("append 0");

        chain
            .append(
                AuditEvent {
                    event_id: Uuid::now_v7(),
                    timestamp_utc: Utc::now(),
                    session_id,
                    device_pubkey: host.public_key(),
                    actor_pubkey: viewer,
                    server_domain: "support.example.org".to_owned(),
                    kind: AuditEventKind::GrantIssued {
                        permissions: GrantPermissions::default(),
                    },
                },
                &host,
            )
            .expect("append 1");

        assert!(chain.verify(&host.public_key()).is_ok());

        // Tamper with event 0 domain
        let mut tampered = chain.entries().to_vec();
        tampered[0].event.server_domain = "evil.example.org".to_owned();
        assert!(matches!(
            verify_chain(&tampered, &host.public_key()),
            Err(ProtocolError::AuditChainBroken { index: 0, .. })
        ));

        // Delete entry 0 (sequence gap / prev_hash mismatch)
        assert!(matches!(
            verify_chain(&chain.entries()[1..], &host.public_key()),
            Err(ProtocolError::AuditChainBroken { index: 0, .. })
        ));
    }

    #[test]
    fn hash_chain_detects_single_byte_tamper_in_middle_entry() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let session_id = Uuid::now_v7();
        let mut chain = AuditChain::new();

        for kind in [
            AuditEventKind::SessionRequested,
            AuditEventKind::FileTransferMetadata {
                filename: "report.pdf".to_owned(),
                size_bytes: 1024,
            },
            AuditEventKind::SessionRevoked {
                reason: RevocationReason::HostEnded,
                duration_secs: 42,
            },
        ] {
            chain
                .append(
                    AuditEvent {
                        event_id: Uuid::now_v7(),
                        timestamp_utc: Utc::now(),
                        session_id,
                        device_pubkey: host.public_key(),
                        actor_pubkey: viewer,
                        server_domain: "support.example.org".to_owned(),
                        kind,
                    },
                    &host,
                )
                .expect("append entry");
        }
        assert!(chain.verify(&host.public_key()).is_ok());

        // Flip 1 byte in middle entry #1's entry_hash
        let mut tampered_hash = chain.entries().to_vec();
        tampered_hash[1].entry_hash[15] ^= 0x01;
        assert!(matches!(
            verify_chain(&tampered_hash, &host.public_key()),
            Err(ProtocolError::AuditChainBroken { index: 1, .. })
        ));

        // Flip 1 byte in middle entry #1's metadata payload (size_bytes 1024 -> 1025)
        let mut tampered_payload = chain.entries().to_vec();
        tampered_payload[1].event.kind = AuditEventKind::FileTransferMetadata {
            filename: "report.pdf".to_owned(),
            size_bytes: 1025,
        };
        assert!(matches!(
            verify_chain(&tampered_payload, &host.public_key()),
            Err(ProtocolError::AuditChainBroken { index: 1, .. })
        ));
    }
}
