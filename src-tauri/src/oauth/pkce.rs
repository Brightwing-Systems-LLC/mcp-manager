use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Generate a PKCE code verifier (43-128 characters, base64url-encoded random bytes).
pub fn generate_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Compute the PKCE code challenge (base64url-encoded SHA-256 of verifier).
pub fn compute_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

/// Generate a random state parameter for CSRF protection.
pub fn generate_state() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_length() {
        let v = generate_verifier();
        assert!(v.len() >= 43 && v.len() <= 128, "Verifier length: {}", v.len());
    }

    #[test]
    fn test_verifier_is_base64url() {
        let v = generate_verifier();
        assert!(v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_challenge_is_sha256_of_verifier() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_challenge(verifier);
        // SHA-256 of the above is known:
        let expected_hash = Sha256::digest(verifier.as_bytes());
        let expected = URL_SAFE_NO_PAD.encode(expected_hash);
        assert_eq!(challenge, expected);
    }

    #[test]
    fn test_challenge_length() {
        let v = generate_verifier();
        let c = compute_challenge(&v);
        assert_eq!(c.len(), 43, "SHA-256 base64url is always 43 chars");
    }

    #[test]
    fn test_state_uniqueness() {
        let s1 = generate_state();
        let s2 = generate_state();
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_state_length() {
        let s = generate_state();
        assert_eq!(s.len(), 43);
    }
}
