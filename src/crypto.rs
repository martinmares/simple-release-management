use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine};
use rand::Rng;

/// Encrypt a plaintext string using AES-256-GCM
/// Returns base64-encoded string in format: nonce||ciphertext
pub fn encrypt(plaintext: &str, secret_key: &str) -> Result<String> {
    // Derive 32-byte key from secret
    let key = derive_key(secret_key);

    // Create cipher instance
    let cipher = Aes256Gcm::new(&key.into());

    // Generate random 12-byte nonce
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; 12] = rng.gen();
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt the plaintext
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

    // Combine nonce and ciphertext
    let mut result = Vec::new();
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    // Encode to base64
    Ok(general_purpose::STANDARD.encode(&result))
}

/// Decrypt a base64-encoded encrypted string
/// Expected format: nonce||ciphertext
pub fn decrypt(encrypted: &str, secret_key: &str) -> Result<String> {
    // Decode from base64
    let encrypted_bytes = general_purpose::STANDARD
        .decode(encrypted)
        .context("Invalid base64 encoding")?;

    // Extract nonce (first 12 bytes) and ciphertext
    if encrypted_bytes.len() < 12 {
        anyhow::bail!("Encrypted data too short");
    }

    let (nonce_bytes, ciphertext) = encrypted_bytes.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Derive key
    let key = derive_key(secret_key);

    // Create cipher instance
    let cipher = Aes256Gcm::new(&key.into());

    // Decrypt
    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("Decryption failed"))?;

    // Convert to string
    String::from_utf8(plaintext_bytes).context("Invalid UTF-8 in decrypted data")
}

/// Derive a 32-byte key from the secret string using SHA-256
fn derive_key(secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    result.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let secret = "test-secret-key-must-be-secure";
        let plaintext = "my-password-123";

        let encrypted = encrypt(plaintext, secret).unwrap();
        assert_ne!(encrypted, plaintext);

        let decrypted = decrypt(&encrypted, secret).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_nonces() {
        let secret = "test-secret-key";
        let plaintext = "same-password";

        let encrypted1 = encrypt(plaintext, secret).unwrap();
        let encrypted2 = encrypt(plaintext, secret).unwrap();

        // Different nonces should produce different ciphertexts
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same plaintext
        assert_eq!(decrypt(&encrypted1, secret).unwrap(), plaintext);
        assert_eq!(decrypt(&encrypted2, secret).unwrap(), plaintext);
    }

    #[test]
    fn test_wrong_key() {
        let plaintext = "secret-data";
        let encrypted = encrypt(plaintext, "correct-key").unwrap();

        let result = decrypt(&encrypted, "wrong-key");
        assert!(result.is_err());
    }
}
