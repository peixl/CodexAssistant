//! Process-wide helper token for the local HTTP bridge.
//!
//! Generated once per process launch via getrandom; used to gate
//! `127.0.0.1:<helper_port>` requests so only Codex renderer pages
//! (which receive the token via CDP injection) can call the bridge.

use std::sync::OnceLock;

use base64::Engine;

const TOKEN_BYTES: usize = 32;

static TOKEN: OnceLock<String> = OnceLock::new();

pub fn ensure_helper_token() -> &'static str {
    TOKEN.get_or_init(generate_token)
}

fn generate_token() -> String {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::getrandom(&mut bytes).expect("OS RNG must succeed");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn verify_token(provided: &str) -> bool {
    let expected = ensure_helper_token().as_bytes();
    let provided = provided.as_bytes();
    if provided.len() != expected.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in expected.iter().zip(provided.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_43_chars_url_safe_base64() {
        let token = ensure_helper_token();
        assert_eq!(token.len(), 43);
        assert!(
            token
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        );
    }

    #[test]
    fn token_is_stable_within_process() {
        let first = ensure_helper_token();
        let second = ensure_helper_token();
        assert_eq!(first, second);
    }

    #[test]
    fn verify_token_accepts_real_token() {
        let token = ensure_helper_token().to_string();
        assert!(verify_token(&token));
    }

    #[test]
    fn verify_token_rejects_wrong_length() {
        assert!(!verify_token(""));
        assert!(!verify_token("a"));
        assert!(!verify_token(&"a".repeat(42)));
        assert!(!verify_token(&"a".repeat(44)));
    }

    #[test]
    fn verify_token_rejects_same_length_mismatch() {
        let mut bad = ensure_helper_token().to_string();
        // flip the last char to something definitely different
        let last = bad.pop().unwrap();
        bad.push(if last == 'A' { 'B' } else { 'A' });
        assert!(!verify_token(&bad));
    }
}
