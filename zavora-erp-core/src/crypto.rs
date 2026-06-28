//! Symmetric encryption for secrets stored at rest (notification provider
//! credentials: SMTP password, SMS API key, Twilio auth token).
//!
//! Uses AES-256-GCM (AEAD) with a deployment key supplied via the
//! `NOTIF_ENC_KEY` environment variable (base64-encoded 32 bytes). Each
//! encryption generates a fresh random 96-bit nonce, returned alongside the
//! ciphertext; both are persisted (`secret_enc`, `secret_nonce`). The plaintext
//! secret is never written to the database or returned by the API.
//!
//! Generate a key with:  `openssl rand -base64 32`

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

use crate::error::{ErpError, ErpResult};

const KEY_ENV: &str = "NOTIF_ENC_KEY";

/// Load the 32-byte AES key from `NOTIF_ENC_KEY` (base64). Returns a clear error
/// when the key is missing or malformed, so callers can surface an actionable
/// message ("set NOTIF_ENC_KEY to store provider secrets").
fn load_key() -> ErpResult<Key<Aes256Gcm>> {
    let raw = std::env::var(KEY_ENV).map_err(|_| ErpError::ValidationFailed {
        message: format!(
            "{KEY_ENV} is not set; cannot store or read encrypted provider secrets. \
             Generate one with `openssl rand -base64 32`."
        ),
    })?;
    key_from_b64(&raw)
}

/// Parse a base64-encoded 32-byte key. Pure (no env), so it is unit-testable.
fn key_from_b64(raw: &str) -> ErpResult<Key<Aes256Gcm>> {
    let bytes = B64
        .decode(raw.trim())
        .map_err(|_| ErpError::ValidationFailed {
            message: format!("{KEY_ENV} must be valid base64"),
        })?;
    if bytes.len() != 32 {
        return Err(ErpError::ValidationFailed {
            message: format!("{KEY_ENV} must decode to exactly 32 bytes (got {})", bytes.len()),
        });
    }
    Ok(*Key::<Aes256Gcm>::from_slice(&bytes))
}

/// `true` when an encryption key is configured (used to gate secret writes with
/// a friendly error before attempting them).
pub fn encryption_available() -> bool {
    load_key().is_ok()
}

/// Encrypt `plaintext`, returning `(ciphertext, nonce)`. Both are stored.
pub fn encrypt(plaintext: &str) -> ErpResult<(Vec<u8>, Vec<u8>)> {
    encrypt_with(&load_key()?, plaintext)
}

/// Decrypt a `(ciphertext, nonce)` pair back to the plaintext secret.
pub fn decrypt(ciphertext: &[u8], nonce: &[u8]) -> ErpResult<String> {
    decrypt_with(&load_key()?, ciphertext, nonce)
}

fn encrypt_with(key: &Key<Aes256Gcm>, plaintext: &str) -> ErpResult<(Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| ErpError::Internal("failed to encrypt secret".to_string()))?;
    Ok((ciphertext, nonce.to_vec()))
}

fn decrypt_with(key: &Key<Aes256Gcm>, ciphertext: &[u8], nonce: &[u8]) -> ErpResult<String> {
    let cipher = Aes256Gcm::new(key);
    if nonce.len() != 12 {
        return Err(ErpError::Internal("stored nonce has wrong length".to_string()));
    }
    let nonce = Nonce::from_slice(nonce);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| ErpError::Internal(
            "failed to decrypt secret (wrong NOTIF_ENC_KEY or corrupt data)".to_string(),
        ))?;
    String::from_utf8(plaintext)
        .map_err(|_| ErpError::Internal("decrypted secret is not valid UTF-8".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two distinct, valid 32-byte base64 keys for tests (no env mutation).
    const KEY_A: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";
    const KEY_B: &str = "ZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmY=";

    #[test]
    fn roundtrip_with_key() {
        let key = key_from_b64(KEY_A).unwrap();
        let (ct, nonce) = encrypt_with(&key, "super-secret-token").expect("encrypt");
        assert_ne!(ct, b"super-secret-token");
        let pt = decrypt_with(&key, &ct, &nonce).expect("decrypt");
        assert_eq!(pt, "super-secret-token");
    }

    #[test]
    fn distinct_nonces_per_encryption() {
        let key = key_from_b64(KEY_A).unwrap();
        let (_, n1) = encrypt_with(&key, "x").unwrap();
        let (_, n2) = encrypt_with(&key, "x").unwrap();
        assert_ne!(n1, n2, "each encryption must use a fresh nonce");
    }

    #[test]
    fn wrong_key_fails_decrypt() {
        let ka = key_from_b64(KEY_A).unwrap();
        let kb = key_from_b64(KEY_B).unwrap();
        let (ct, nonce) = encrypt_with(&ka, "hello").unwrap();
        assert!(decrypt_with(&kb, &ct, &nonce).is_err());
    }

    #[test]
    fn key_validation_rejects_bad_input() {
        assert!(key_from_b64("not base64!!!").is_err());
        assert!(key_from_b64("c2hvcnQ=").is_err()); // "short" → too few bytes
        assert!(key_from_b64(KEY_A).is_ok());
    }
}

