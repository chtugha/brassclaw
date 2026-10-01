//! Internal SHA-256 checksum helpers for component content integrity.

/// Compute the canonical SHA-256 hex digest of a component's prose field.
///
/// Applied to:
/// - `reborn_skills.body`
/// - `reborn_tool_skills.content`
/// - `reborn_python_code.content`
pub(crate) fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(s.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_always_64_chars() {
        assert_eq!(sha256_hex("hello world").len(), 64);
        assert_eq!(sha256_hex("").len(), 64);
        assert_eq!(sha256_hex("some longer text content").len(), 64);
    }

    #[test]
    fn sha256_hex_of_empty_string() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hex_is_deterministic() {
        assert_eq!(sha256_hex("hello"), sha256_hex("hello"));
    }
}
