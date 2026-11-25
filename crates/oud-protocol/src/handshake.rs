//! Authenticated ephemeral X25519 + Ed25519 key exchange and local
//! commit-before-reveal 6-digit Short Authentication String (SAS) derivation.

use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::ProtocolError;
use crate::identity::{IdentityKeyPair, IdentityPublicKey, IdentitySignature};

const COMMIT_DOMAIN: &[u8] = b"oudos-sas-commit-v1";
const OFFER_DOMAIN: &[u8] = b"oudos-handshake-offer-v1";
const ACCEPT_DOMAIN: &[u8] = b"oudos-handshake-accept-v1";
const TRANSCRIPT_DOMAIN: &[u8] = b"oudos-transcript-v1";
const SAS_HKDF_INFO: &[u8] = b"oudos-sas-v1";
const SESSION_KEY_HKDF_INFO: &[u8] = b"oudos-session-key-v1";

/// Maximum failed 6-digit SAS attempts before requester lockout.
pub const MAX_SAS_MISMATCHES: u32 = 3;

/// Ephemeral X25519 keypair for a single session handshake.
pub struct EphemeralKeyPair {
    secret: EphemeralSecret,
    public: PublicKey,
}

impl EphemeralKeyPair {
    /// Generates a fresh ephemeral X25519 keypair using OS CSPRNG.
    #[must_use]
    pub fn generate() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Returns the 32-byte public X25519 key.
    #[must_use]
    pub fn public_bytes(&self) -> [u8; 32] {
        self.public.to_bytes()
    }

    /// Consumes the secret to compute the Diffie-Hellman shared secret.
    #[must_use]
    pub fn diffie_hellman(self, peer_public: [u8; 32]) -> [u8; 32] {
        let peer = PublicKey::from(peer_public);
        self.secret.diffie_hellman(&peer).to_bytes()
    }
}

/// Viewer's authenticated ephemeral offer with commit-before-reveal SAS commitment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeOffer {
    /// Unique session identifier.
    pub session_id: Uuid,
    /// Raw enrolled server domain (e.g. `support.example.org`).
    pub server_domain: String,
    /// Viewer's Ed25519 identity public key.
    pub viewer_identity: IdentityPublicKey,
    /// Viewer's ephemeral X25519 public key.
    pub viewer_ephemeral: [u8; 32],
    /// Commitment `SHA-256(COMMIT_DOMAIN || viewer_ephemeral || commit_nonce)`.
    pub sas_commitment: [u8; 32],
    /// Ed25519 signature by `viewer_identity` over the offer fields.
    pub viewer_signature: IdentitySignature,
}

/// Host's authenticated ephemeral acceptance bound to the offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeAccept {
    /// Unique session identifier matching the offer.
    pub session_id: Uuid,
    /// Raw enrolled server domain matching the offer.
    pub server_domain: String,
    /// Host's Ed25519 identity public key.
    pub host_identity: IdentityPublicKey,
    /// Host's ephemeral X25519 public key.
    pub host_ephemeral: [u8; 32],
    /// Ed25519 signature by `host_identity` binding both ephemeral keys and commitment.
    pub host_signature: IdentitySignature,
}

/// Viewer's secret nonce revealed only after receiving [`HandshakeAccept`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SasReveal {
    /// 32-byte random commitment nonce.
    pub commit_nonce: [u8; 32],
}

/// Completed local handshake output on either peer.
#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct HandshakeOutcome {
    /// SHA-256 transcript hash binding identities, ephemeral keys, commitment, and DH secret.
    pub transcript_hash: [u8; 32],
    /// 6-digit verification code (`"000000"`..`"999999"`), derived locally.
    pub sas_code: String,
    /// 32-byte forward-secret session encryption key.
    pub session_key: [u8; 32],
}

/// Initiator state held while awaiting [`HandshakeAccept`].
pub struct InitiatorPending {
    ephemeral: EphemeralKeyPair,
    offer: HandshakeOffer,
    reveal: SasReveal,
}

impl InitiatorPending {
    /// Creates an authenticated [`HandshakeOffer`] and commitment for `session_id`.
    #[must_use]
    pub fn initiate(
        session_id: Uuid,
        server_domain: &str,
        viewer_keys: &IdentityKeyPair,
    ) -> (Self, HandshakeOffer) {
        let ephemeral = EphemeralKeyPair::generate();
        let viewer_ephemeral = ephemeral.public_bytes();
        let mut commit_nonce = [0u8; 32];
        OsRng.fill_bytes(&mut commit_nonce);

        let sas_commitment = compute_commitment(&viewer_ephemeral, &commit_nonce);
        let viewer_identity = viewer_keys.public_key();
        let offer_bytes = offer_signing_bytes(
            session_id,
            server_domain,
            &viewer_identity,
            &viewer_ephemeral,
            &sas_commitment,
        );
        let viewer_signature = viewer_keys.sign(&offer_bytes);

        let offer = HandshakeOffer {
            session_id,
            server_domain: server_domain.to_owned(),
            viewer_identity,
            viewer_ephemeral,
            sas_commitment,
            viewer_signature,
        };
        let reveal = SasReveal { commit_nonce };
        let pending = Self {
            ephemeral,
            offer: offer.clone(),
            reveal,
        };
        (pending, offer)
    }

    /// Completes the initiator side upon receiving a valid [`HandshakeAccept`],
    /// returning the [`SasReveal`] and locally derived [`HandshakeOutcome`].
    ///
    /// # Errors
    /// Fails closed if server domain, session ID, or host signature fails verification.
    pub fn complete(
        self,
        accept: &HandshakeAccept,
        expected_host_identity: Option<&IdentityPublicKey>,
    ) -> Result<(SasReveal, HandshakeOutcome), ProtocolError> {
        if accept.session_id != self.offer.session_id
            || accept.server_domain != self.offer.server_domain
        {
            return Err(ProtocolError::GrantBindingMismatch);
        }
        if let Some(expected) = expected_host_identity {
            if expected != &accept.host_identity {
                return Err(ProtocolError::HostKeyChanged);
            }
        }
        let accept_bytes = accept_signing_bytes(
            accept.session_id,
            &accept.server_domain,
            &self.offer.viewer_identity,
            &accept.host_identity,
            &self.offer.viewer_ephemeral,
            &accept.host_ephemeral,
            &self.offer.sas_commitment,
        );
        accept
            .host_identity
            .verify(&accept_bytes, &accept.host_signature)?;

        let mut dh_secret = self.ephemeral.diffie_hellman(accept.host_ephemeral);
        let outcome = finalize_outcome(
            self.offer.session_id,
            &self.offer.server_domain,
            &self.offer.viewer_identity,
            &accept.host_identity,
            &self.offer.viewer_ephemeral,
            &accept.host_ephemeral,
            &self.offer.sas_commitment,
            &self.reveal.commit_nonce,
            &dh_secret,
        )?;
        dh_secret.zeroize();
        Ok((self.reveal, outcome))
    }
}

/// Responder state held after issuing [`HandshakeAccept`] until [`SasReveal`] arrives.
pub struct ResponderPending {
    offer: HandshakeOffer,
    host_identity: IdentityPublicKey,
    host_ephemeral: [u8; 32],
    dh_secret: [u8; 32],
}

impl Drop for ResponderPending {
    fn drop(&mut self) {
        self.dh_secret.zeroize();
    }
}

impl ResponderPending {
    /// Verifies the viewer's [`HandshakeOffer`] and produces [`HandshakeAccept`].
    ///
    /// # Errors
    /// Returns [`ProtocolError::SignatureVerificationFailed`] if the offer signature is invalid.
    pub fn respond(
        offer: &HandshakeOffer,
        host_keys: &IdentityKeyPair,
    ) -> Result<(Self, HandshakeAccept), ProtocolError> {
        let offer_bytes = offer_signing_bytes(
            offer.session_id,
            &offer.server_domain,
            &offer.viewer_identity,
            &offer.viewer_ephemeral,
            &offer.sas_commitment,
        );
        offer
            .viewer_identity
            .verify(&offer_bytes, &offer.viewer_signature)?;

        let ephemeral = EphemeralKeyPair::generate();
        let host_ephemeral = ephemeral.public_bytes();
        let host_identity = host_keys.public_key();
        let accept_bytes = accept_signing_bytes(
            offer.session_id,
            &offer.server_domain,
            &offer.viewer_identity,
            &host_identity,
            &offer.viewer_ephemeral,
            &host_ephemeral,
            &offer.sas_commitment,
        );
        let host_signature = host_keys.sign(&accept_bytes);
        let dh_secret = ephemeral.diffie_hellman(offer.viewer_ephemeral);

        let accept = HandshakeAccept {
            session_id: offer.session_id,
            server_domain: offer.server_domain.clone(),
            host_identity,
            host_ephemeral,
            host_signature,
        };
        let pending = Self {
            offer: offer.clone(),
            host_identity,
            host_ephemeral,
            dh_secret,
        };
        Ok((pending, accept))
    }

    /// Verifies the viewer's [`SasReveal`] against the prior commitment and derives
    /// the local 6-digit SAS and session key.
    ///
    /// # Errors
    /// Returns [`ProtocolError::CommitmentMismatch`] if the reveal does not match the offer commitment.
    pub fn verify_reveal_and_finalize(
        self,
        reveal: &SasReveal,
    ) -> Result<HandshakeOutcome, ProtocolError> {
        let expected_commit =
            compute_commitment(&self.offer.viewer_ephemeral, &reveal.commit_nonce);
        if expected_commit != self.offer.sas_commitment {
            return Err(ProtocolError::CommitmentMismatch);
        }

        finalize_outcome(
            self.offer.session_id,
            &self.offer.server_domain,
            &self.offer.viewer_identity,
            &self.host_identity,
            &self.offer.viewer_ephemeral,
            &self.host_ephemeral,
            &self.offer.sas_commitment,
            &reveal.commit_nonce,
            &self.dh_secret,
        )
    }
}

/// Derives the 6-digit verification code from a transcript hash using:
/// `Truncate_6-digit(HKDF-SHA256(transcript_hash, "oudos-sas-v1"))`.
///
/// # Errors
/// Returns [`ProtocolError::KeyDerivationFailed`] if HKDF expansion fails.
pub fn derive_sas_6digit(transcript_hash: &[u8; 32]) -> Result<String, ProtocolError> {
    let hk = Hkdf::<Sha256>::new(None, transcript_hash);
    let mut okm = [0u8; 4];
    hk.expand(SAS_HKDF_INFO, &mut okm)
        .map_err(|_| ProtocolError::KeyDerivationFailed)?;
    let code = u32::from_be_bytes(okm) % 1_000_000;
    Ok(format!("{code:06}"))
}

fn compute_commitment(viewer_ephemeral: &[u8; 32], commit_nonce: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(COMMIT_DOMAIN);
    hasher.update(viewer_ephemeral);
    hasher.update(commit_nonce);
    hasher.finalize().into()
}

fn len_u32_be(s: &str) -> [u8; 4] {
    u32::try_from(s.len()).unwrap_or(u32::MAX).to_be_bytes()
}

fn offer_signing_bytes(
    session_id: Uuid,
    server_domain: &str,
    viewer_identity: &IdentityPublicKey,
    viewer_ephemeral: &[u8; 32],
    sas_commitment: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(OFFER_DOMAIN);
    out.extend_from_slice(session_id.as_bytes());
    out.extend_from_slice(&len_u32_be(server_domain));
    out.extend_from_slice(server_domain.as_bytes());
    out.extend_from_slice(viewer_identity.as_bytes());
    out.extend_from_slice(viewer_ephemeral);
    out.extend_from_slice(sas_commitment);
    out
}

fn accept_signing_bytes(
    session_id: Uuid,
    server_domain: &str,
    viewer_identity: &IdentityPublicKey,
    host_identity: &IdentityPublicKey,
    viewer_ephemeral: &[u8; 32],
    host_ephemeral: &[u8; 32],
    sas_commitment: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(224);
    out.extend_from_slice(ACCEPT_DOMAIN);
    out.extend_from_slice(session_id.as_bytes());
    out.extend_from_slice(&len_u32_be(server_domain));
    out.extend_from_slice(server_domain.as_bytes());
    out.extend_from_slice(viewer_identity.as_bytes());
    out.extend_from_slice(host_identity.as_bytes());
    out.extend_from_slice(viewer_ephemeral);
    out.extend_from_slice(host_ephemeral);
    out.extend_from_slice(sas_commitment);
    out
}

#[allow(clippy::too_many_arguments)]
fn finalize_outcome(
    session_id: Uuid,
    server_domain: &str,
    viewer_identity: &IdentityPublicKey,
    host_identity: &IdentityPublicKey,
    viewer_ephemeral: &[u8; 32],
    host_ephemeral: &[u8; 32],
    sas_commitment: &[u8; 32],
    commit_nonce: &[u8; 32],
    dh_secret: &[u8; 32],
) -> Result<HandshakeOutcome, ProtocolError> {
    let mut hasher = Sha256::new();
    hasher.update(TRANSCRIPT_DOMAIN);
    hasher.update(session_id.as_bytes());
    hasher.update(len_u32_be(server_domain));
    hasher.update(server_domain.as_bytes());
    hasher.update(viewer_identity.as_bytes());
    hasher.update(host_identity.as_bytes());
    hasher.update(viewer_ephemeral);
    hasher.update(host_ephemeral);
    hasher.update(sas_commitment);
    hasher.update(commit_nonce);
    hasher.update(dh_secret);
    let transcript_hash: [u8; 32] = hasher.finalize().into();

    let sas_code = derive_sas_6digit(&transcript_hash)?;
    let hk = Hkdf::<Sha256>::new(Some(&transcript_hash), dh_secret);
    let mut session_key = [0u8; 32];
    hk.expand(SESSION_KEY_HKDF_INFO, &mut session_key)
        .map_err(|_| ProtocolError::KeyDerivationFailed)?;

    Ok(HandshakeOutcome {
        transcript_hash,
        sas_code,
        session_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_before_reveal_derives_identical_sas_and_session_key() {
        let viewer = IdentityKeyPair::generate();
        let host = IdentityKeyPair::generate();
        let session_id = Uuid::now_v7();

        let (init_pending, offer) =
            InitiatorPending::initiate(session_id, "support.example.org", &viewer);
        let (resp_pending, accept) =
            ResponderPending::respond(&offer, &host).expect("offer verifies");
        let (reveal, viewer_out) = init_pending
            .complete(&accept, Some(&host.public_key()))
            .expect("accept verifies");
        let host_out = resp_pending
            .verify_reveal_and_finalize(&reveal)
            .expect("reveal matches commit");

        assert_eq!(viewer_out.transcript_hash, host_out.transcript_hash);
        assert_eq!(viewer_out.sas_code, host_out.sas_code);
        assert_eq!(viewer_out.sas_code.len(), 6);
        assert_eq!(viewer_out.session_key, host_out.session_key);
    }

    #[test]
    fn tampered_reveal_fails_closed() {
        let viewer = IdentityKeyPair::generate();
        let host = IdentityKeyPair::generate();
        let session_id = Uuid::now_v7();

        let (init_pending, offer) =
            InitiatorPending::initiate(session_id, "support.example.org", &viewer);
        let (resp_pending, accept) =
            ResponderPending::respond(&offer, &host).expect("offer verifies");
        let (mut reveal, _viewer_out) = init_pending
            .complete(&accept, Some(&host.public_key()))
            .expect("accept verifies");
        reveal.commit_nonce[0] ^= 0x01;

        assert_eq!(
            resp_pending.verify_reveal_and_finalize(&reveal),
            Err(ProtocolError::CommitmentMismatch)
        );
    }

    #[test]
    fn transcript_binds_commit_nonce_against_pre_reveal_guessing() {
        let viewer = IdentityKeyPair::generate();
        let host = IdentityKeyPair::generate();
        let session_id = Uuid::now_v7();

        let (init_pending, offer) =
            InitiatorPending::initiate(session_id, "support.example.org", &viewer);
        let (_resp_pending, accept) =
            ResponderPending::respond(&offer, &host).expect("offer verifies");
        let (reveal, viewer_out) = init_pending
            .complete(&accept, Some(&host.public_key()))
            .expect("accept verifies");

        let mut wrong_nonce = reveal.commit_nonce;
        wrong_nonce[31] ^= 0xFF;
        let forged_outcome = finalize_outcome(
            session_id,
            "support.example.org",
            &viewer.public_key(),
            &host.public_key(),
            &offer.viewer_ephemeral,
            &accept.host_ephemeral,
            &offer.sas_commitment,
            &wrong_nonce,
            &[0x42; 32],
        )
        .expect("outcome derives");

        assert_ne!(forged_outcome.transcript_hash, viewer_out.transcript_hash);
    }
}
