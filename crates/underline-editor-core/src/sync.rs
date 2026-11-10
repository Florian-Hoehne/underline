//! ADR-025: what keeps the webview's and the backend's copies of an open
//! note's text alike (`docs/migration/variante-b.md`, section 3): the edits
//! as sets over a text in chars of the rope ([`ChangeSet`], [`Change`]),
//! transformed over each other with the backend's first at the same place;
//! the backend's mirror with its revisions ([`Server`]); the webview's side
//! ([`Client`]); a backend change applied to the window's editor as egui
//! does ([`apply_remote`]); and the hash both sides compute the same way,
//! whatever the platform or the Rust version: FNV-1a with 64 bits.

mod apply;
#[cfg(test)]
mod cases;
mod changeset;
mod client;
mod server;

pub use apply::{apply_remote, shifted};
pub use changeset::{Change, ChangeSet, OutOfRange};
pub use client::{Client, Desync, Incoming};
pub use server::{Answer, Batch, Received, Remote, Resynced, Server, Touched, KEPT};

/// The FNV-1a offset basis for 64 bits.
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// The FNV-1a prime for 64 bits.
const PRIME: u64 = 0x0100_0000_01b3;

/// FNV-1a with 64 bits, over bytes and over fields of text.
///
/// Unlike `std`'s `DefaultHasher`, its values are fixed, so the webview
/// (WASM) and the backend make the same key of the same content.
///
/// Two different contents may get the same 64-bit hash. For the keys of
/// colours and diagrams that is accepted: among the few thousand blocks a
/// note holds, a collision has a chance of about 1 in 10^12, and it would
/// show one block in the colours or the picture of another until it is
/// edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fnv1a(u64);

impl Default for Fnv1a {
    fn default() -> Self {
        Self(OFFSET)
    }
}

impl Fnv1a {
    /// A hash of nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes in `bytes` as they are.
    #[must_use]
    pub fn bytes(mut self, bytes: &[u8]) -> Self {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(PRIME);
        }
        self
    }

    /// Takes in `text` as one field: its length in bytes first, so
    /// `("ab", "c")` and `("a", "bc")` hash differently.
    #[must_use]
    pub fn field(self, text: &str) -> Self {
        let length = u64::try_from(text.len()).unwrap_or(u64::MAX);
        self.bytes(&length.to_le_bytes()).bytes(text.as_bytes())
    }

    /// The hash.
    #[must_use]
    pub fn finish(self) -> u64 {
        self.0
    }

    /// The hash as 16 hex digits. Keys that cross to the window take this
    /// form: a JavaScript number holds only 53 bits exactly.
    #[must_use]
    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

/// ADR-025: the hash of `text` (its UTF-8 bytes) as 16 hex digits, which
/// the webview and the backend compare to find out that their texts differ.
#[must_use]
pub fn hash(text: &str) -> String {
    Fnv1a::new().bytes(text.as_bytes()).hex()
}

/// [`hash`] of the text of `rope`, taken chunk by chunk without copying it.
#[must_use]
pub fn hash_rope(rope: &ropey::Rope) -> String {
    rope.chunks()
        .fold(Fnv1a::new(), |hash, chunk| hash.bytes(chunk.as_bytes()))
        .hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-025: the values are FNV-1a's published test vectors, so they
    /// never change with the platform or the Rust version.
    #[test]
    fn adr025_fnv1a_gives_the_published_values() {
        assert_eq!(hash(""), "cbf29ce484222325");
        assert_eq!(hash("a"), "af63dc4c8601ec8c");
        assert_eq!(hash("foobar"), "85944171f73967e8");
    }

    /// ADR-025: fields are told apart by their lengths.
    #[test]
    fn adr025_fields_are_told_apart() {
        let ab_c = Fnv1a::new().field("ab").field("c").finish();
        let a_bc = Fnv1a::new().field("a").field("bc").finish();
        assert_ne!(ab_c, a_bc);
    }
}
