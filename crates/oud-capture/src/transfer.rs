//! Directional text clipboard sync and chunked opt-in file transfer gated by
//! [`HostFrameGatekeeper`] and `SessionGrant` permissions.
//!
//! Both capabilities remain **OFF by default** and fail closed whenever unchecked
//! in the active signed [`oud_protocol::grant::SessionGrant`]. Audit helpers emit
//! metadata only (`filename`, `size_bytes`, usage event) and never log payloads.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use oud_protocol::audit::AuditEventKind;
use oud_protocol::grant::{ClipboardDirection, FrameKind};
use serde::{Deserialize, Serialize};

use crate::error::CaptureError;
use crate::gatekeeper::HostFrameGatekeeper;

/// Default maximum file transfer size cap (100 MiB) when enabled.
pub const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// Maximum UTF-8 byte length permitted for a single text clipboard sync frame (64 KiB).
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 64 * 1024;

/// Default chunk size for WebRTC data-channel file transfer (16 KiB).
pub const DEFAULT_TRANSFER_CHUNK_SIZE: usize = 16 * 1024;

/// Maximum allowable chunk size per packet (256 KiB).
pub const MAX_TRANSFER_CHUNK_SIZE: usize = 256 * 1024;

/// Maximum concurrent in-flight file transfers per session.
pub const MAX_CONCURRENT_TRANSFERS: usize = 4;

/// Executable/script extensions blocked by default in `FileTransferLimits`.
pub const DEFAULT_BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "bat", "cmd", "com", "msi", "ps1", "psm1", "vbs", "vbe", "js", "jse", "hta", "scr",
    "pif", "dll", "sys", "reg",
];

/// Origin endpoint for a directional clipboard sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardEndpoint {
    /// Local host clipboard pushing to viewer.
    Host,
    /// Remote viewer clipboard pushing to host.
    Viewer,
}

/// Metadata-only record produced when a clipboard sync frame is authorized (never text contents).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardAuditMetadata {
    /// Configured direction at the time of transfer.
    pub direction: ClipboardDirection,
    /// Origin endpoint.
    pub sender: ClipboardEndpoint,
    /// UTF-8 byte length (metadata only).
    pub byte_len: usize,
}

impl ClipboardAuditMetadata {
    /// Converts to the metadata-only protocol audit event kind.
    #[must_use]
    pub const fn to_audit_kind(self) -> AuditEventKind {
        AuditEventKind::ClipboardSyncEvent
    }
}

/// Enforces directional, text-only clipboard synchronization behind [`HostFrameGatekeeper`].
#[derive(Debug, Clone)]
pub struct ClipboardSyncManager {
    direction: ClipboardDirection,
    max_text_bytes: usize,
}

impl Default for ClipboardSyncManager {
    fn default() -> Self {
        Self::new(ClipboardDirection::Disabled)
    }
}

impl ClipboardSyncManager {
    /// Creates a new clipboard manager with explicit direction (use [`ClipboardDirection::Disabled`]
    /// as the fail-closed default).
    #[must_use]
    pub const fn new(direction: ClipboardDirection) -> Self {
        Self {
            direction,
            max_text_bytes: MAX_CLIPBOARD_TEXT_BYTES,
        }
    }

    /// Returns the configured clipboard sync direction.
    #[must_use]
    pub const fn direction(&self) -> ClipboardDirection {
        self.direction
    }

    /// Updates the directional policy (caller must enforce local-host origin).
    pub fn set_direction(&mut self, direction: ClipboardDirection) {
        self.direction = direction;
    }

    /// Authorizes and validates a UTF-8 text clipboard sync frame against `gatekeeper`,
    /// direction control, and size/content constraints.
    ///
    /// # Errors
    /// Fails closed with [`CaptureError::FrameBlockedByGrant`] if `clipboard_sync` is unchecked
    /// in `SessionGrant` (or banner unmounted / grant expired / revoked), or
    /// [`CaptureError::ClipboardSyncBlocked`] if direction/text checks fail.
    pub fn authorize_text_sync(
        &self,
        gatekeeper: &mut HostFrameGatekeeper,
        sender: ClipboardEndpoint,
        text: &str,
        now: DateTime<Utc>,
    ) -> Result<ClipboardAuditMetadata, CaptureError> {
        gatekeeper.authorize_frame(FrameKind::ClipboardSync, now)?;

        let dir_allowed = match sender {
            ClipboardEndpoint::Host => self.direction.allows_host_to_viewer(),
            ClipboardEndpoint::Viewer => self.direction.allows_viewer_to_host(),
        };
        if !dir_allowed {
            return Err(CaptureError::ClipboardSyncBlocked(format!(
                "clipboard direction {:?} blocks sender {:?}",
                self.direction, sender
            )));
        }

        if text.is_empty() {
            return Err(CaptureError::ClipboardSyncBlocked(
                "clipboard text payload must not be empty".to_owned(),
            ));
        }
        if text.len() > self.max_text_bytes {
            return Err(CaptureError::ClipboardSyncBlocked(format!(
                "clipboard text ({} B) exceeds {} B limit",
                text.len(),
                self.max_text_bytes
            )));
        }
        if text.bytes().any(|b| b == 0) {
            return Err(CaptureError::ClipboardSyncBlocked(
                "clipboard text must not contain NUL bytes".to_owned(),
            ));
        }

        gatekeeper.record_activity(now);
        Ok(ClipboardAuditMetadata {
            direction: self.direction,
            sender,
            byte_len: text.len(),
        })
    }
}

/// Size, chunking, and extension restrictions for opt-in chunked file transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTransferLimits {
    /// Maximum allowed file size in bytes (bounded by `oud-policy` `max_file_size_bytes`).
    pub max_file_size_bytes: u64,
    /// Maximum byte payload per WebRTC data-channel chunk.
    pub chunk_size_bytes: usize,
    /// Blocked file extensions (lowercase without leading dot).
    pub blocked_extensions: Vec<String>,
    /// Optional strict allowlist of extensions (lowercase without leading dot).
    pub allowed_extensions: Option<Vec<String>>,
}

impl Default for FileTransferLimits {
    fn default() -> Self {
        Self {
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
            chunk_size_bytes: DEFAULT_TRANSFER_CHUNK_SIZE,
            blocked_extensions: DEFAULT_BLOCKED_EXTENSIONS
                .iter()
                .map(|ext| (*ext).to_owned())
                .collect(),
            allowed_extensions: None,
        }
    }
}

impl FileTransferLimits {
    /// Validates and normalizes a bare filename (no paths/traversal) and declared byte size.
    ///
    /// # Errors
    /// Returns [`CaptureError::FileTransferRejected`] on empty names, path traversal,
    /// disallowed extensions, zero-byte files, or sizes exceeding `max_file_size_bytes`.
    pub fn validate_file_spec(
        &self,
        raw_filename: &str,
        declared_size_bytes: u64,
    ) -> Result<String, CaptureError> {
        let filename = sanitize_transfer_filename(raw_filename)?;
        if self.chunk_size_bytes == 0 || self.chunk_size_bytes > MAX_TRANSFER_CHUNK_SIZE {
            return Err(CaptureError::FileTransferRejected(
                "invalid file transfer chunk size".to_owned(),
            ));
        }
        if declared_size_bytes == 0 {
            return Err(CaptureError::FileTransferRejected(
                "file size must be greater than 0 bytes".to_owned(),
            ));
        }
        if declared_size_bytes > self.max_file_size_bytes {
            return Err(CaptureError::FileTransferRejected(format!(
                "file `{filename}` ({declared_size_bytes} B) exceeds {} B limit",
                self.max_file_size_bytes
            )));
        }

        let ext = Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);

        if let Some(ref ext_val) = ext {
            if self
                .blocked_extensions
                .iter()
                .any(|blocked| blocked.eq_ignore_ascii_case(ext_val))
            {
                return Err(CaptureError::FileTransferRejected(format!(
                    "file extension `.{ext_val}` is blocked by safety limits"
                )));
            }
        }

        if let Some(allowed) = &self.allowed_extensions {
            let ok = ext
                .as_deref()
                .is_some_and(|e| allowed.iter().any(|item| item.eq_ignore_ascii_case(e)));
            if !ok {
                return Err(CaptureError::FileTransferRejected(format!(
                    "file `{filename}` extension is not in the permitted allowlist"
                )));
            }
        }

        Ok(filename)
    }
}

/// Result returned by an optional [`FileScanHook`] before committing a completed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileScanVerdict {
    /// Payload passed optional scan checks.
    Clean,
    /// Payload rejected by scan hook with a plain-language reason.
    Rejected {
        /// Plain-language scan rejection reason.
        reason: String,
    },
}

/// Pluggable scan hook invoked after all chunks arrive and before committing the file.
pub trait FileScanHook: Send + Sync {
    /// Inspects the assembled payload bytes for `filename` (`size_bytes`).
    ///
    /// # Errors
    /// Returns [`CaptureError::FileTransferRejected`] if the scanner itself errors.
    fn scan_payload(
        &self,
        filename: &str,
        size_bytes: u64,
        payload: &[u8],
    ) -> Result<FileScanVerdict, CaptureError>;
}

/// Default no-op scan hook when no external AV/CDR scanner is attached.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopFileScanHook;

impl FileScanHook for NoopFileScanHook {
    fn scan_payload(
        &self,
        _filename: &str,
        _size_bytes: u64,
        _payload: &[u8],
    ) -> Result<FileScanVerdict, CaptureError> {
        Ok(FileScanVerdict::Clean)
    }
}

/// Lifecycle state of an individual chunked file transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FileTransferStatus {
    /// Actively receiving or sending chunks.
    InProgress,
    /// All chunks received, size verified, and scan hook passed.
    Completed,
    /// Transfer cancelled mid-flight; partial buffer discarded.
    Cancelled {
        /// Cancellation reason.
        reason: String,
    },
    /// Transfer rejected due to grant revocation, limits, or scan verdict.
    Rejected {
        /// Rejection reason.
        reason: String,
    },
}

/// Progress snapshot for a chunked file transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTransferProgress {
    /// Transfer identifier.
    pub transfer_id: String,
    /// Sanitized basename.
    pub filename: String,
    /// Declared total byte size.
    pub total_bytes: u64,
    /// Bytes transferred so far.
    pub transferred_bytes: u64,
    /// Chunks received so far.
    pub chunks_received: u32,
    /// Expected total chunk count.
    pub total_chunks: u32,
    /// Current lifecycle state.
    pub status: FileTransferStatus,
}

impl FileTransferProgress {
    /// Returns integer percentage (`0..=100`).
    #[must_use]
    pub fn progress_percent(&self) -> u8 {
        if self.total_bytes == 0 {
            return 0;
        }
        let pct = self
            .transferred_bytes
            .saturating_mul(100)
            .checked_div(self.total_bytes)
            .unwrap_or(0)
            .min(100);
        u8::try_from(pct).unwrap_or(100)
    }
}

/// Finalized file transfer output paired with metadata-only audit event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedFileTransfer {
    /// Transfer identifier.
    pub transfer_id: String,
    /// Sanitized basename.
    pub filename: String,
    /// Verified payload byte size.
    pub size_bytes: u64,
    /// Verified file payload bytes (never logged).
    pub payload: Vec<u8>,
    /// Metadata-only audit event (`filename` + `size_bytes`, never payload).
    pub audit_metadata: AuditEventKind,
}

/// Outbound/inbound chunk packet for chunked data-channel transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChunkPacket {
    /// Transfer identifier.
    pub transfer_id: String,
    /// Zero-based sequential chunk index.
    pub seq: u32,
    /// Chunk byte payload.
    pub data: Vec<u8>,
}

/// Splits `payload` into sequential [`FileChunkPacket`] items bounded by `chunk_size_bytes`.
///
/// # Errors
/// Returns [`CaptureError::FileTransferRejected`] if `chunk_size_bytes == 0` or exceeds
/// [`MAX_TRANSFER_CHUNK_SIZE`].
pub fn chunk_file_bytes(
    transfer_id: &str,
    payload: &[u8],
    chunk_size_bytes: usize,
) -> Result<Vec<FileChunkPacket>, CaptureError> {
    if chunk_size_bytes == 0 || chunk_size_bytes > MAX_TRANSFER_CHUNK_SIZE {
        return Err(CaptureError::FileTransferRejected(
            "invalid chunk size".to_owned(),
        ));
    }
    let mut packets = Vec::new();
    for (idx, chunk) in payload.chunks(chunk_size_bytes).enumerate() {
        let seq = u32::try_from(idx)
            .map_err(|_| CaptureError::FileTransferRejected("chunk count overflow".to_owned()))?;
        packets.push(FileChunkPacket {
            transfer_id: transfer_id.to_owned(),
            seq,
            data: chunk.to_vec(),
        });
    }
    Ok(packets)
}

#[derive(Debug, Clone)]
struct ActiveTransferBuffer {
    filename: String,
    total_bytes: u64,
    total_chunks: u32,
    next_seq: u32,
    buffer: Vec<u8>,
    status: FileTransferStatus,
}

impl ActiveTransferBuffer {
    fn to_progress(&self, transfer_id: &str) -> FileTransferProgress {
        FileTransferProgress {
            transfer_id: transfer_id.to_owned(),
            filename: self.filename.clone(),
            total_bytes: self.total_bytes,
            transferred_bytes: self.buffer.len() as u64,
            chunks_received: self.next_seq,
            total_chunks: self.total_chunks,
            status: self.status.clone(),
        }
    }
}

/// Manages chunked opt-in file transfers with [`HostFrameGatekeeper`] enforcement,
/// size/type limits, mid-transfer cancellation, and optional [`FileScanHook`] inspection.
pub struct FileTransferManager {
    limits: FileTransferLimits,
    scan_hook: Arc<dyn FileScanHook>,
    transfers: HashMap<String, ActiveTransferBuffer>,
}

impl Default for FileTransferManager {
    fn default() -> Self {
        Self::new(FileTransferLimits::default(), Arc::new(NoopFileScanHook))
    }
}

impl FileTransferManager {
    /// Creates a new file transfer manager with explicit limits and scan hook.
    #[must_use]
    pub fn new(limits: FileTransferLimits, scan_hook: Arc<dyn FileScanHook>) -> Self {
        Self {
            limits,
            scan_hook,
            transfers: HashMap::new(),
        }
    }

    /// Returns the configured [`FileTransferLimits`].
    #[must_use]
    pub const fn limits(&self) -> &FileTransferLimits {
        &self.limits
    }

    /// Updates `max_file_size_bytes` (e.g. from monotonic policy ceiling).
    pub fn set_max_file_size_bytes(&mut self, max_bytes: u64) {
        self.limits.max_file_size_bytes = max_bytes;
    }

    /// Initiates a new chunked file transfer after enforcing `FrameKind::FileTransfer` on
    /// `gatekeeper` and validating filename/size/extension limits.
    ///
    /// # Errors
    /// Fails closed on grant refusal (`FrameBlockedByGrant`) or invalid file metadata.
    pub fn start_transfer(
        &mut self,
        gatekeeper: &mut HostFrameGatekeeper,
        transfer_id: impl Into<String>,
        raw_filename: &str,
        declared_size_bytes: u64,
        now: DateTime<Utc>,
    ) -> Result<FileTransferProgress, CaptureError> {
        gatekeeper.authorize_frame(FrameKind::FileTransfer, now)?;

        let transfer_id = transfer_id.into();
        if transfer_id.trim().is_empty() {
            return Err(CaptureError::FileTransferRejected(
                "transfer_id must not be empty".to_owned(),
            ));
        }

        let in_flight_count = self
            .transfers
            .values()
            .filter(|t| matches!(t.status, FileTransferStatus::InProgress))
            .count();
        if in_flight_count >= MAX_CONCURRENT_TRANSFERS {
            return Err(CaptureError::FileTransferRejected(format!(
                "maximum concurrent transfers ({MAX_CONCURRENT_TRANSFERS}) reached"
            )));
        }
        if self.transfers.contains_key(&transfer_id) {
            return Err(CaptureError::FileTransferRejected(format!(
                "duplicate transfer_id `{transfer_id}`"
            )));
        }

        let filename = self
            .limits
            .validate_file_spec(raw_filename, declared_size_bytes)?;
        let chunk_u64 = self.limits.chunk_size_bytes as u64;
        let total_chunks_u64 = declared_size_bytes.div_ceil(chunk_u64);
        let total_chunks = u32::try_from(total_chunks_u64)
            .map_err(|_| CaptureError::FileTransferRejected("too many chunks".to_owned()))?;

        let cap = usize::try_from(declared_size_bytes).map_err(|_| {
            CaptureError::FileTransferRejected("size exceeds addressable RAM".to_owned())
        })?;

        let entry = ActiveTransferBuffer {
            filename,
            total_bytes: declared_size_bytes,
            total_chunks,
            next_seq: 0,
            buffer: Vec::with_capacity(cap.min(256 * 1024)),
            status: FileTransferStatus::InProgress,
        };
        let progress = entry.to_progress(&transfer_id);
        self.transfers.insert(transfer_id, entry);
        gatekeeper.record_activity(now);
        Ok(progress)
    }

    /// Appends a sequential data chunk to an active transfer.
    ///
    /// Re-verifies `gatekeeper.authorize_frame(FrameKind::FileTransfer, now)` on **every** chunk:
    /// if the host toggles `file_transfer` off or revokes the session mid-transfer, the in-flight
    /// buffer is discarded immediately and the transfer transitions to `Rejected`.
    ///
    /// # Errors
    /// Fails closed on grant refusal, unknown/inactive transfer, bad chunk sequence, or size overflow.
    pub fn push_chunk(
        &mut self,
        gatekeeper: &mut HostFrameGatekeeper,
        transfer_id: &str,
        seq: u32,
        data: &[u8],
        now: DateTime<Utc>,
    ) -> Result<FileTransferProgress, CaptureError> {
        if let Err(err) = gatekeeper.authorize_frame(FrameKind::FileTransfer, now) {
            if let Some(entry) = self.transfers.get_mut(transfer_id) {
                entry.buffer.clear();
                entry.status = FileTransferStatus::Rejected {
                    reason: "file_transfer permission disabled or grant revoked".to_owned(),
                };
            }
            return Err(err);
        }

        let Some(entry) = self.transfers.get_mut(transfer_id) else {
            return Err(CaptureError::FileTransferRejected(format!(
                "unknown transfer_id `{transfer_id}`"
            )));
        };

        if !matches!(entry.status, FileTransferStatus::InProgress) {
            return Err(CaptureError::FileTransferRejected(format!(
                "transfer `{transfer_id}` is no longer in progress"
            )));
        }

        if data.is_empty() || data.len() > self.limits.chunk_size_bytes {
            entry.buffer.clear();
            entry.status = FileTransferStatus::Rejected {
                reason: "chunk length out of bounds".to_owned(),
            };
            return Err(CaptureError::FileTransferRejected(
                "chunk length out of bounds".to_owned(),
            ));
        }

        if seq != entry.next_seq {
            entry.buffer.clear();
            entry.status = FileTransferStatus::Rejected {
                reason: format!("unexpected chunk seq {seq}, expected {}", entry.next_seq),
            };
            return Err(CaptureError::FileTransferRejected(format!(
                "unexpected chunk seq {seq}, expected {}",
                entry.next_seq
            )));
        }

        let new_len = (entry.buffer.len() as u64).saturating_add(data.len() as u64);
        if new_len > entry.total_bytes || new_len > self.limits.max_file_size_bytes {
            entry.buffer.clear();
            entry.status = FileTransferStatus::Rejected {
                reason: "payload exceeded declared or policy size limit".to_owned(),
            };
            return Err(CaptureError::FileTransferRejected(
                "payload exceeded declared or policy size limit".to_owned(),
            ));
        }

        entry.buffer.extend_from_slice(data);
        entry.next_seq = entry.next_seq.saturating_add(1);
        gatekeeper.record_activity(now);
        Ok(entry.to_progress(transfer_id))
    }

    /// Cancels an in-flight transfer mid-stream and discards any partial buffered bytes.
    pub fn cancel_transfer(
        &mut self,
        transfer_id: &str,
        reason: impl Into<String>,
    ) -> Option<FileTransferProgress> {
        let entry = self.transfers.get_mut(transfer_id)?;
        if matches!(entry.status, FileTransferStatus::InProgress) {
            entry.buffer.clear();
            entry.status = FileTransferStatus::Cancelled {
                reason: reason.into(),
            };
        }
        Some(entry.to_progress(transfer_id))
    }

    /// Verifies exact total byte completion, re-checks `HostFrameGatekeeper`, runs the
    /// optional [`FileScanHook`], and returns the completed file + metadata-only audit event.
    ///
    /// # Errors
    /// Fails closed on grant refusal, incomplete transfer, or scan rejection.
    pub fn finish_transfer(
        &mut self,
        gatekeeper: &mut HostFrameGatekeeper,
        transfer_id: &str,
        now: DateTime<Utc>,
    ) -> Result<CompletedFileTransfer, CaptureError> {
        if let Err(err) = gatekeeper.authorize_frame(FrameKind::FileTransfer, now) {
            if let Some(entry) = self.transfers.get_mut(transfer_id) {
                entry.buffer.clear();
                entry.status = FileTransferStatus::Rejected {
                    reason: "file_transfer permission disabled or grant revoked".to_owned(),
                };
            }
            return Err(err);
        }

        let Some(entry) = self.transfers.get_mut(transfer_id) else {
            return Err(CaptureError::FileTransferRejected(format!(
                "unknown transfer_id `{transfer_id}`"
            )));
        };

        if !matches!(entry.status, FileTransferStatus::InProgress) {
            return Err(CaptureError::FileTransferRejected(format!(
                "transfer `{transfer_id}` is not in progress"
            )));
        }

        let actual_len = entry.buffer.len() as u64;
        if actual_len != entry.total_bytes {
            entry.buffer.clear();
            entry.status = FileTransferStatus::Rejected {
                reason: format!(
                    "incomplete payload: got {actual_len} B, expected {} B",
                    entry.total_bytes
                ),
            };
            return Err(CaptureError::FileTransferRejected(format!(
                "incomplete payload: got {actual_len} B, expected {} B",
                entry.total_bytes
            )));
        }

        match self
            .scan_hook
            .scan_payload(&entry.filename, actual_len, &entry.buffer)?
        {
            FileScanVerdict::Clean => {}
            FileScanVerdict::Rejected { reason } => {
                entry.buffer.clear();
                entry.status = FileTransferStatus::Rejected {
                    reason: reason.clone(),
                };
                return Err(CaptureError::FileTransferRejected(format!(
                    "scan hook rejected `{}`: {reason}",
                    entry.filename
                )));
            }
        }

        let payload = std::mem::take(&mut entry.buffer);
        entry.status = FileTransferStatus::Completed;
        let filename = entry.filename.clone();

        Ok(CompletedFileTransfer {
            transfer_id: transfer_id.to_owned(),
            filename: filename.clone(),
            size_bytes: actual_len,
            payload,
            audit_metadata: AuditEventKind::FileTransferMetadata {
                filename,
                size_bytes: actual_len,
            },
        })
    }

    /// Returns progress snapshot for `transfer_id`, if tracked.
    #[must_use]
    pub fn progress(&self, transfer_id: &str) -> Option<FileTransferProgress> {
        self.transfers
            .get(transfer_id)
            .map(|entry| entry.to_progress(transfer_id))
    }
}

fn sanitize_transfer_filename(raw: &str) -> Result<String, CaptureError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 255
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(char::is_control)
    {
        return Err(CaptureError::FileTransferRejected(
            "invalid or unsafe filename".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oud_protocol::grant::{GrantDuration, GrantPermissions, SessionGrant};
    use oud_protocol::identity::IdentityKeyPair;
    use uuid::Uuid;

    struct RejectKeywordScanHook {
        blocked_marker: Vec<u8>,
    }

    impl FileScanHook for RejectKeywordScanHook {
        fn scan_payload(
            &self,
            _filename: &str,
            _size_bytes: u64,
            payload: &[u8],
        ) -> Result<FileScanVerdict, CaptureError> {
            if payload
                .windows(self.blocked_marker.len())
                .any(|w| w == self.blocked_marker.as_slice())
            {
                Ok(FileScanVerdict::Rejected {
                    reason: "flagged test signature".to_owned(),
                })
            } else {
                Ok(FileScanVerdict::Clean)
            }
        }
    }

    fn make_gatekeeper(
        perms: GrantPermissions,
        now: DateTime<Utc>,
    ) -> (IdentityKeyPair, SessionGrant, HostFrameGatekeeper) {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let sid = Uuid::now_v7();
        let grant = SessionGrant::issue(
            &host,
            sid,
            viewer,
            "support.example.org",
            perms,
            GrantDuration::OneHour,
            now,
        )
        .expect("grant issued");
        let gk = HostFrameGatekeeper::from_verified_grant(
            grant.clone(),
            host.public_key(),
            sid,
            viewer,
            "support.example.org",
            now,
        )
        .expect("gatekeeper");
        (host, grant, gk)
    }

    #[test]
    fn clipboard_sync_is_off_by_default_and_respects_direction_control() {
        let now = Utc::now();
        let (_host, _grant, mut screen_only_gk) = make_gatekeeper(GrantPermissions::default(), now);
        let mut mgr = ClipboardSyncManager::new(ClipboardDirection::Bidirectional);

        // Fail closed whenever clipboard_sync is false in SessionGrant.
        assert!(matches!(
            mgr.authorize_text_sync(&mut screen_only_gk, ClipboardEndpoint::Host, "hello", now),
            Err(CaptureError::FrameBlockedByGrant {
                frame_kind: "clipboard_sync"
            })
        ));
        assert_eq!(screen_only_gk.dropped_frames(), 1);

        // With clipboard_sync enabled in grant, direction control is still enforced.
        let (_host2, _grant2, mut clip_gk) = make_gatekeeper(
            GrantPermissions {
                clipboard_sync: true,
                ..GrantPermissions::default()
            },
            now,
        );
        mgr.set_direction(ClipboardDirection::Disabled);
        assert!(matches!(
            mgr.authorize_text_sync(&mut clip_gk, ClipboardEndpoint::Host, "secret", now),
            Err(CaptureError::ClipboardSyncBlocked(_))
        ));

        mgr.set_direction(ClipboardDirection::HostToViewer);
        assert!(matches!(
            mgr.authorize_text_sync(&mut clip_gk, ClipboardEndpoint::Viewer, "from viewer", now),
            Err(CaptureError::ClipboardSyncBlocked(_))
        ));
        let meta = mgr
            .authorize_text_sync(&mut clip_gk, ClipboardEndpoint::Host, "from host", now)
            .expect("host->viewer allowed");
        assert_eq!(meta.byte_len, 9);
        assert_eq!(meta.to_audit_kind(), AuditEventKind::ClipboardSyncEvent);
        // Verify audit JSON contains no clipboard text content.
        let audit_json = serde_json::to_string(&meta.to_audit_kind()).expect("serialize");
        assert!(!audit_json.contains("from host"));

        mgr.set_direction(ClipboardDirection::ViewerToHost);
        assert!(mgr
            .authorize_text_sync(&mut clip_gk, ClipboardEndpoint::Viewer, "to host", now)
            .is_ok());
        assert!(mgr
            .authorize_text_sync(&mut clip_gk, ClipboardEndpoint::Host, "blocked", now)
            .is_err());

        // Reject NUL bytes and oversized payloads.
        mgr.set_direction(ClipboardDirection::Bidirectional);
        assert!(mgr
            .authorize_text_sync(&mut clip_gk, ClipboardEndpoint::Host, "a\0b", now)
            .is_err());
        let oversized = "x".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert!(mgr
            .authorize_text_sync(&mut clip_gk, ClipboardEndpoint::Host, &oversized, now)
            .is_err());
    }

    #[test]
    fn file_transfer_fails_closed_when_unchecked_and_enforces_limits() {
        let now = Utc::now();
        let (_host, _grant, mut screen_only_gk) = make_gatekeeper(GrantPermissions::default(), now);
        let mut mgr = FileTransferManager::default();

        // Off by default -> fail closed at HostFrameGatekeeper.
        assert!(matches!(
            mgr.start_transfer(&mut screen_only_gk, "tx-0", "notes.txt", 10, now),
            Err(CaptureError::FrameBlockedByGrant {
                frame_kind: "file_transfer"
            })
        ));
        assert_eq!(screen_only_gk.dropped_frames(), 1);

        let (_host, _grant, mut ft_gk) = make_gatekeeper(
            GrantPermissions {
                file_transfer: true,
                ..GrantPermissions::default()
            },
            now,
        );
        let mut mgr = FileTransferManager::new(
            FileTransferLimits {
                max_file_size_bytes: 1024,
                chunk_size_bytes: 16,
                ..FileTransferLimits::default()
            },
            Arc::new(NoopFileScanHook),
        );

        assert!(mgr
            .start_transfer(&mut ft_gk, "tx-exe", "payload.exe", 64, now)
            .is_err());
        assert!(mgr
            .start_transfer(&mut ft_gk, "tx-trav", "../secret.txt", 64, now)
            .is_err());
        assert!(mgr
            .start_transfer(&mut ft_gk, "tx-big", "huge.pdf", 2048, now)
            .is_err());

        let payload = b"Oudos chunked file transfer test payload!".to_vec();
        let packets = chunk_file_bytes("tx-1", &payload, 16).expect("chunk");
        assert_eq!(packets.len(), 3);

        let p0 = mgr
            .start_transfer(&mut ft_gk, "tx-1", "report.txt", payload.len() as u64, now)
            .expect("start tx-1");
        assert_eq!(p0.progress_percent(), 0);
        assert_eq!(p0.total_chunks, 3);

        let p1 = mgr
            .push_chunk(&mut ft_gk, "tx-1", packets[0].seq, &packets[0].data, now)
            .expect("chunk 0");
        assert_eq!(p1.chunks_received, 1);
        assert!(p1.progress_percent() > 0 && p1.progress_percent() < 100);

        mgr.push_chunk(&mut ft_gk, "tx-1", packets[1].seq, &packets[1].data, now)
            .expect("chunk 1");
        mgr.push_chunk(&mut ft_gk, "tx-1", packets[2].seq, &packets[2].data, now)
            .expect("chunk 2");
        let completed = mgr
            .finish_transfer(&mut ft_gk, "tx-1", now)
            .expect("finish tx-1");
        assert_eq!(completed.payload, payload);
        assert_eq!(
            completed.audit_metadata,
            AuditEventKind::FileTransferMetadata {
                filename: "report.txt".to_owned(),
                size_bytes: payload.len() as u64,
            }
        );
    }

    #[test]
    fn file_transfer_supports_mid_transfer_cancel_and_mid_session_revocation() {
        let now = Utc::now();
        let (host, grant, mut ft_gk) = make_gatekeeper(
            GrantPermissions {
                file_transfer: true,
                ..GrantPermissions::default()
            },
            now,
        );
        let mut mgr = FileTransferManager::new(
            FileTransferLimits {
                max_file_size_bytes: 1024,
                chunk_size_bytes: 16,
                ..FileTransferLimits::default()
            },
            Arc::new(NoopFileScanHook),
        );

        // Mid-transfer cancel discards partial buffer.
        let p_cancel_start = mgr
            .start_transfer(&mut ft_gk, "tx-cancel", "temp.txt", 32, now)
            .expect("start tx-cancel");
        assert_eq!(p_cancel_start.status, FileTransferStatus::InProgress);
        mgr.push_chunk(&mut ft_gk, "tx-cancel", 0, b"0123456789abcdef", now)
            .expect("chunk 0");
        let cancelled = mgr
            .cancel_transfer("tx-cancel", "user clicked cancel")
            .expect("cancelled");
        assert!(matches!(
            cancelled.status,
            FileTransferStatus::Cancelled { .. }
        ));
        assert!(mgr
            .push_chunk(&mut ft_gk, "tx-cancel", 1, b"0123456789abcdef", now)
            .is_err());

        // Mid-session toggle-off of file_transfer immediately aborts in-flight transfers.
        mgr.start_transfer(&mut ft_gk, "tx-mid", "mid.txt", 32, now)
            .expect("start tx-mid");
        mgr.push_chunk(&mut ft_gk, "tx-mid", 0, b"0123456789abcdef", now)
            .expect("chunk 0");
        let revoked_ft = grant
            .reissue_with_permissions(
                &host,
                GrantPermissions::default(),
                now + chrono::Duration::seconds(2),
            )
            .expect("reissue");
        ft_gk
            .update_grant(revoked_ft, now + chrono::Duration::seconds(2))
            .expect("update grant");
        assert!(matches!(
            mgr.push_chunk(
                &mut ft_gk,
                "tx-mid",
                1,
                b"0123456789abcdef",
                now + chrono::Duration::seconds(3)
            ),
            Err(CaptureError::FrameBlockedByGrant {
                frame_kind: "file_transfer"
            })
        ));
        assert!(matches!(
            mgr.progress("tx-mid").map(|p| p.status),
            Some(FileTransferStatus::Rejected { .. })
        ));
    }

    #[test]
    fn file_scan_hook_rejects_flagged_payload_and_discards_buffer() {
        let now = Utc::now();
        let (_host, _grant, mut ft_gk) = make_gatekeeper(
            GrantPermissions {
                file_transfer: true,
                ..GrantPermissions::default()
            },
            now,
        );
        let hook = Arc::new(RejectKeywordScanHook {
            blocked_marker: b"EICAR-TEST".to_vec(),
        });
        let mut mgr = FileTransferManager::new(FileTransferLimits::default(), hook);

        let bad_payload = b"prefix-EICAR-TEST-suffix";
        mgr.start_transfer(
            &mut ft_gk,
            "tx-scan",
            "doc.txt",
            bad_payload.len() as u64,
            now,
        )
        .expect("start");
        mgr.push_chunk(&mut ft_gk, "tx-scan", 0, bad_payload, now)
            .expect("push");
        assert!(matches!(
            mgr.finish_transfer(&mut ft_gk, "tx-scan", now),
            Err(CaptureError::FileTransferRejected(_))
        ));
        assert!(matches!(
            mgr.progress("tx-scan").map(|p| p.status),
            Some(FileTransferStatus::Rejected { .. })
        ));
    }
}
