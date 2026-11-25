//! Device Ed25519 identity keys, 9-digit display locators, and Trust-On-First-Use
//! (TOFU) host key pinning.

use core::fmt;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroize;

use crate::error::ProtocolError;

const PUBKEY_PREFIX: &str = "ed25519:";
const SIG_PREFIX: &str = "ed25519_sig:";

/// 9-digit display locator for a device (e.g. `482 190 734`).
///
/// Display locators are **never** authentication tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId([u8; 9]);

impl DeviceId {
    /// Parses a 9-digit device ID, ignoring spaces and hyphens.
    ///
    /// # Errors
    /// Returns [`ProtocolError::InvalidDeviceId`] if the normalized string does
    /// not contain exactly 9 ASCII digits.
    pub fn parse(input: &str) -> Result<Self, ProtocolError> {
        let digits: Vec<u8> = input.bytes().filter(|b| *b != b' ' && *b != b'-').collect();
        if digits.len() != 9 || !digits.iter().all(u8::is_ascii_digit) {
            return Err(ProtocolError::InvalidDeviceId);
        }
        let mut arr = [0u8; 9];
        arr.copy_from_slice(&digits);
        Ok(Self(arr))
    }

    /// Formats the 9-digit ID in grouped `XXX XXX XXX` display form.
    #[must_use]
    pub fn display_grouped(&self) -> String {
        let s = core::str::from_utf8(&self.0).unwrap_or("000000000");
        format!("{} {} {}", &s[0..3], &s[3..6], &s[6..9])
    }

    /// Returns the compact 9-digit string without spaces.
    #[must_use]
    pub fn as_compact(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("000000000")
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_grouped())
    }
}

impl Serialize for DeviceId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_compact())
    }
}

impl<'de> Deserialize<'de> for DeviceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// 32-byte Ed25519 public identity key, serialized as `ed25519:<hex>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdentityPublicKey([u8; 32]);

impl IdentityPublicKey {
    /// Constructs an [`IdentityPublicKey`] from raw 32-byte Ed25519 compressed bytes.
    ///
    /// # Errors
    /// Returns [`ProtocolError::InvalidPublicKey`] if the bytes are not a valid Ed25519 point.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, ProtocolError> {
        VerifyingKey::from_bytes(&bytes).map_err(|_| ProtocolError::InvalidPublicKey)?;
        Ok(Self(bytes))
    }

    /// Returns the raw 32-byte key.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Formats as `ed25519:<lowercase-hex>`.
    #[must_use]
    pub fn to_prefixed_hex(&self) -> String {
        format!("{PUBKEY_PREFIX}{}", hex::encode(self.0))
    }

    /// Parses `ed25519:<hex>`.
    ///
    /// # Errors
    /// Returns [`ProtocolError::InvalidPublicKey`] on invalid prefix, hex, or curve point.
    pub fn from_prefixed_hex(input: &str) -> Result<Self, ProtocolError> {
        let raw_hex = input
            .strip_prefix(PUBKEY_PREFIX)
            .ok_or(ProtocolError::InvalidPublicKey)?;
        let decoded = hex::decode(raw_hex).map_err(|_| ProtocolError::InvalidPublicKey)?;
        let arr: [u8; 32] = decoded
            .try_into()
            .map_err(|_| ProtocolError::InvalidPublicKey)?;
        Self::from_bytes(arr)
    }

    /// Verifies an Ed25519 signature over `message`.
    ///
    /// # Errors
    /// Returns [`ProtocolError::InvalidPublicKey`] or [`ProtocolError::SignatureVerificationFailed`].
    pub fn verify(
        &self,
        message: &[u8],
        signature: &IdentitySignature,
    ) -> Result<(), ProtocolError> {
        let vk = VerifyingKey::from_bytes(&self.0).map_err(|_| ProtocolError::InvalidPublicKey)?;
        let sig = Signature::from_bytes(&signature.0);
        vk.verify(message, &sig)
            .map_err(|_| ProtocolError::SignatureVerificationFailed)
    }
}

impl fmt::Display for IdentityPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_prefixed_hex())
    }
}

impl Serialize for IdentityPublicKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_prefixed_hex())
    }
}

impl<'de> Deserialize<'de> for IdentityPublicKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_prefixed_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// 64-byte Ed25519 signature, serialized as `ed25519_sig:<hex>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentitySignature([u8; 64]);

impl IdentitySignature {
    /// Wraps raw 64-byte Ed25519 signature bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    /// Returns the raw 64-byte signature.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }

    /// Formats as `ed25519_sig:<lowercase-hex>`.
    #[must_use]
    pub fn to_prefixed_hex(&self) -> String {
        format!("{SIG_PREFIX}{}", hex::encode(self.0))
    }

    /// Parses `ed25519_sig:<hex>`.
    ///
    /// # Errors
    /// Returns [`ProtocolError::InvalidSignatureEncoding`] on malformed prefix, hex, or length.
    pub fn from_prefixed_hex(input: &str) -> Result<Self, ProtocolError> {
        let raw_hex = input
            .strip_prefix(SIG_PREFIX)
            .ok_or(ProtocolError::InvalidSignatureEncoding)?;
        let decoded = hex::decode(raw_hex).map_err(|_| ProtocolError::InvalidSignatureEncoding)?;
        let arr: [u8; 64] = decoded
            .try_into()
            .map_err(|_| ProtocolError::InvalidSignatureEncoding)?;
        Ok(Self(arr))
    }
}

impl fmt::Display for IdentitySignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_prefixed_hex())
    }
}

impl Serialize for IdentitySignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_prefixed_hex())
    }
}

impl<'de> Deserialize<'de> for IdentitySignature {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_prefixed_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Ed25519 signing keypair kept in memory and backed by OS secure storage.
pub struct IdentityKeyPair {
    signing_key: SigningKey,
}

impl IdentityKeyPair {
    /// Generates a fresh Ed25519 identity keypair using OS randomness.
    #[must_use]
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Constructs a keypair from a 32-byte secret seed (e.g. loaded from DPAPI/TPM).
    #[must_use]
    pub fn from_seed(mut seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        seed.zeroize();
        Self { signing_key }
    }

    /// Exports the 32-byte secret seed (for OS secure storage only).
    #[must_use]
    pub fn to_seed(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Returns the corresponding [`IdentityPublicKey`].
    #[must_use]
    pub fn public_key(&self) -> IdentityPublicKey {
        IdentityPublicKey(self.signing_key.verifying_key().to_bytes())
    }

    /// Signs `message` with the Ed25519 private key.
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> IdentitySignature {
        let sig = self.signing_key.sign(message);
        IdentitySignature(sig.to_bytes())
    }
}

impl fmt::Debug for IdentityKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentityKeyPair")
            .field("public_key", &self.public_key())
            .finish_non_exhaustive()
    }
}

/// Verifies a host identity key against a viewer's pinned TOFU record.
///
/// If no key is pinned yet (`None`), returns `Ok(presented)` for first-use pinning.
/// If a key was previously pinned and differs, fails closed with [`ProtocolError::HostKeyChanged`].
///
/// # Errors
/// Returns [`ProtocolError::HostKeyChanged`] if `pinned` is `Some(k)` and `k != presented`.
pub fn verify_host_key_tofu(
    pinned: Option<&IdentityPublicKey>,
    presented: &IdentityPublicKey,
) -> Result<IdentityPublicKey, ProtocolError> {
    if let Some(expected) = pinned {
        if expected != presented {
            return Err(ProtocolError::HostKeyChanged);
        }
    }
    Ok(*presented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_parses_and_formats_grouped() {
        let id = DeviceId::parse("482-190-734").expect("valid 9-digit id");
        assert_eq!(id.display_grouped(), "482 190 734");
        assert_eq!(id.as_compact(), "482190734");
        assert!(DeviceId::parse("12345678").is_err());
        assert!(DeviceId::parse("12345678a").is_err());
    }

    #[test]
    fn ed25519_sign_and_verify_roundtrip() {
        let kp = IdentityKeyPair::generate();
        let msg = b"oudos-consent-payload";
        let sig = kp.sign(msg);
        assert!(kp.public_key().verify(msg, &sig).is_ok());
        assert_eq!(
            kp.public_key().verify(b"tampered", &sig),
            Err(ProtocolError::SignatureVerificationFailed)
        );
    }

    #[test]
    fn tofu_rejects_changed_host_key() {
        let host_a = IdentityKeyPair::generate().public_key();
        let host_b = IdentityKeyPair::generate().public_key();
        assert_eq!(verify_host_key_tofu(None, &host_a), Ok(host_a));
        assert_eq!(verify_host_key_tofu(Some(&host_a), &host_a), Ok(host_a));
        assert_eq!(
            verify_host_key_tofu(Some(&host_a), &host_b),
            Err(ProtocolError::HostKeyChanged)
        );
    }
}
