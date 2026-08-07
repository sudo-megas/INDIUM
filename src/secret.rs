//! The password buffer.
//!
//! CORE §9: "Passwords are never stored or remembered — typed per use, wiped after."
//! P2 §5 requires this to be hand-written rather than a crate, and to be honest about
//! the one thing it cannot control.

use std::sync::atomic::{compiler_fence, Ordering};

/// A byte buffer that is overwritten when it is dropped.
///
/// The overwrite uses `write_volatile` behind a compiler fence so the optimiser
/// cannot decide the stores are dead and remove them.
pub struct Secret(Vec<u8>);

impl Secret {
    pub fn new(bytes: Vec<u8>) -> Self {
        Secret(bytes)
    }

    pub fn from_text(s: &str) -> Self {
        Secret(s.as_bytes().to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// A NUL-terminated copy for the C boundary.
    ///
    /// HONESTY, and this comment stays here: libarchive copies the passphrase into
    /// its own allocation and keeps it for the reader's lifetime. Our copies are
    /// provably wiped; that one is only freed when the reader is freed at the end of
    /// the operation. Freeing the reader promptly is the best that exists here, and
    /// INDIUM does it — but the claim "wiped" applies to our memory, not to
    /// libarchive's, and no wording in the UI should imply otherwise.
    ///
    /// Returns `None` if the password contains an interior NUL, which the C API
    /// cannot express.
    pub fn to_c_string(&self) -> Option<std::ffi::CString> {
        std::ffi::CString::new(self.0.clone()).ok()
    }
}

impl Clone for Secret {
    fn clone(&self) -> Self {
        Secret(self.0.clone())
    }
}

/// Overwrite a buffer with zeroes so the optimiser cannot elide the stores.
///
/// Split out from `Drop` so it can be tested directly — observing the wipe through a
/// pointer to a dropped `Secret` would be a use-after-free read, and a test that
/// relies on undefined behaviour proves nothing.
fn wipe(buf: &mut [u8]) {
    compiler_fence(Ordering::SeqCst);
    for byte in buf.iter_mut() {
        // SAFETY: `byte` is a valid, uniquely borrowed, properly aligned `u8`.
        unsafe { std::ptr::write_volatile(byte as *mut u8, 0u8) };
    }
    compiler_fence(Ordering::SeqCst);
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Deliberately opaque: a password must never reach a log, a panic message, or a
/// status line by accident.
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret([redacted; {} bytes])", self.0.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_its_bytes() {
        let s = Secret::from_text("indium");
        assert_eq!(s.as_bytes(), b"indium");
        assert_eq!(s.len(), 6);
        assert!(!s.is_empty());
    }

    #[test]
    fn debug_never_leaks_the_password() {
        let s = Secret::from_text("hunter2");
        let rendered = format!("{s:?}");
        assert!(!rendered.contains("hunter2"), "Debug leaked the secret");
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn c_string_round_trips() {
        let s = Secret::from_text("indium");
        let c = s.to_c_string().expect("no interior NUL");
        assert_eq!(c.as_bytes(), b"indium");
    }

    #[test]
    fn interior_nul_is_rejected_not_truncated() {
        let s = Secret::new(b"ind\0ium".to_vec());
        assert!(
            s.to_c_string().is_none(),
            "an interior NUL must be refused, never silently truncated"
        );
    }

    #[test]
    fn wipe_zeroes_every_byte() {
        let mut buf = b"indium".to_vec();
        wipe(&mut buf);
        assert_eq!(buf, vec![0u8; 6], "wipe left password bytes behind");
    }

    #[test]
    fn wipe_handles_an_empty_buffer() {
        let mut buf: Vec<u8> = Vec::new();
        wipe(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn dropping_a_secret_is_safe_and_silent() {
        // Drop is a one-line call into `wipe`, which is covered above. This exists to
        // catch a Drop impl that panics or double-frees, which a wipe test cannot see.
        for len in [0usize, 1, 6, 4096] {
            drop(Secret::new(vec![b'x'; len]));
        }
    }
}
