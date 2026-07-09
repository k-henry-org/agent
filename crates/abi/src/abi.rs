//! The frozen core-wasm Detector ABI: the export contract and the length-prefixed framing a
//! host uses to read a detector's output.
//!
//! **ABI v0** (see `ARCHITECTURE.md`, decision 001). A detector is a core-wasm module — no
//! component model, no WASI — exporting:
//!
//! - `abi_version() -> i32` — the ABI version the artifact was built against.
//! - `alloc(len: i32) -> i32` — reserve `len` bytes in the module's memory, return a pointer.
//! - `dealloc(ptr: i32, len: i32)` — release a buffer previously returned by `alloc`.
//! - `detect(ptr: i32, len: i32) -> i32` — run detection over the UTF-8 input at
//!   `[ptr, ptr+len)` and return a pointer to a **framed** result buffer.
//!
//! A framed buffer is `[len: u32 little-endian][len bytes of UTF-8 JSON]` — a serialized
//! [`crate::Verdict`]. Plain core-wasm + a hand-rolled length prefix is the lowest common
//! denominator that runs on every wasm host (server, edge, browser) with no transpile step,
//! which is the project's "one artifact, everywhere" wedge.

/// The ABI version this crate speaks. A detector's `abi_version` export must equal this for a
/// host to run it. Bumped only on a breaking change to the export or framing contract; additive
/// capabilities stay under the same version where possible.
pub const ABI_VERSION: i32 = 0;

/// Width, in bytes, of the little-endian `u32` length prefix that frames a result buffer.
pub const LEN_PREFIX_BYTES: usize = 4;

/// The names a conformant detector artifact must export. Host and guest both reference these
/// so the contract has exactly one spelling.
pub mod exports {
    /// `fn abi_version() -> i32`
    pub const ABI_VERSION: &str = "abi_version";
    /// `fn alloc(len: i32) -> i32`
    pub const ALLOC: &str = "alloc";
    /// `fn dealloc(ptr: i32, len: i32)`
    pub const DEALLOC: &str = "dealloc";
    /// `fn detect(ptr: i32, len: i32) -> i32`
    pub const DETECT: &str = "detect";
}

/// An error framing or unframing an ABI buffer, or serializing a [`crate::Verdict`] across it.
///
/// Malformed input is always a value, never a panic — the no-panic discipline holds across the
/// host/artifact trust boundary (a hostile artifact must not be able to crash the host).
#[derive(Debug)]
#[non_exhaustive]
pub enum AbiError {
    /// The buffer is shorter than the length prefix, or shorter than the prefix claims.
    Truncated,
    /// The payload is larger than a `u32` length prefix can describe.
    PayloadTooLarge,
    /// The payload was not valid JSON for the expected type.
    Json(serde_json::Error),
}

impl std::fmt::Display for AbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AbiError::Truncated => f.write_str("ABI buffer is truncated"),
            AbiError::PayloadTooLarge => f.write_str("ABI payload exceeds u32 length prefix"),
            AbiError::Json(e) => write!(f, "ABI payload is not valid JSON: {e}"),
        }
    }
}

impl std::error::Error for AbiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AbiError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for AbiError {
    fn from(e: serde_json::Error) -> Self {
        AbiError::Json(e)
    }
}

/// Frame a payload as `[len: u32 LE][payload]`, ready to hand back across the ABI.
///
/// # Errors
/// [`AbiError::PayloadTooLarge`] if `payload` is longer than [`u32::MAX`] bytes.
pub fn frame(payload: &[u8]) -> Result<Vec<u8>, AbiError> {
    let len = u32::try_from(payload.len()).map_err(|_| AbiError::PayloadTooLarge)?;
    let mut out = Vec::with_capacity(LEN_PREFIX_BYTES + payload.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// Borrow the payload out of a `[len: u32 LE][payload]` buffer, validating the prefix.
///
/// # Errors
/// [`AbiError::Truncated`] if the buffer is shorter than the prefix or than the prefix claims.
pub fn unframe(buf: &[u8]) -> Result<&[u8], AbiError> {
    let prefix = buf.get(..LEN_PREFIX_BYTES).ok_or(AbiError::Truncated)?;
    // `prefix` is exactly LEN_PREFIX_BYTES (4) long, so the array conversion cannot fail.
    let len = u32::from_le_bytes(prefix.try_into().map_err(|_| AbiError::Truncated)?) as usize;
    // `checked_add`: a large prefix must not overflow `usize` on a 32-bit target (this crate also
    // compiles to wasm32, where `usize` is 32-bit) — an overflow would wrap into a valid range and
    // mis-read the payload.
    let end = LEN_PREFIX_BYTES
        .checked_add(len)
        .ok_or(AbiError::Truncated)?;
    buf.get(LEN_PREFIX_BYTES..end).ok_or(AbiError::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trips() {
        let payload = b"hello, verdict";
        let framed = frame(payload).unwrap();
        assert_eq!(framed.len(), LEN_PREFIX_BYTES + payload.len());
        assert_eq!(unframe(&framed).unwrap(), payload);
    }

    #[test]
    fn frame_handles_empty() {
        let framed = frame(b"").unwrap();
        assert_eq!(unframe(&framed).unwrap(), b"");
    }

    #[test]
    fn unframe_rejects_short_prefix() {
        assert!(matches!(unframe(&[0, 0]), Err(AbiError::Truncated)));
    }

    #[test]
    fn unframe_rejects_lying_length() {
        // prefix claims 10 bytes, only 2 follow
        let buf = [10u8, 0, 0, 0, 1, 2];
        assert!(matches!(unframe(&buf), Err(AbiError::Truncated)));
    }
}
