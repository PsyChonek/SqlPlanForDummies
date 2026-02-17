use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use sha2::{Digest, Sha256};

fn derive_key() -> [u8; 32] {
    let hostname = hostname::get()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let username = whoami::username();
    let material = format!("SqlPlanForDummies:{}:{}", hostname, username);

    let mut hasher = Sha256::new();
    hasher.update(material.as_bytes());
    let result = hasher.finalize();

    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

pub fn encrypt_password(password: &str) -> Result<String, String> {
    let key = derive_key();
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, password.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

pub fn decrypt_password(encrypted: &str) -> Result<String, String> {
    let key = derive_key();
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;

    let combined = BASE64.decode(encrypted).map_err(|e| e.to_string())?;
    if combined.len() < 12 {
        return Err("Invalid encrypted data".into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| e.to_string())?;

    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = "MySecretPassword123!";

        let encrypted = encrypt_password(original).expect("Encryption should succeed");

        // Encrypted should be different from original
        assert_ne!(encrypted, original);

        // Encrypted should be base64 (no panic on decode)
        assert!(BASE64.decode(&encrypted).is_ok());

        let decrypted = decrypt_password(&encrypted).expect("Decryption should succeed");

        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let password = "SamePassword";

        let encrypted1 = encrypt_password(password).unwrap();
        let encrypted2 = encrypt_password(password).unwrap();

        // Different nonces should produce different ciphertext
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same value
        assert_eq!(decrypt_password(&encrypted1).unwrap(), password);
        assert_eq!(decrypt_password(&encrypted2).unwrap(), password);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let result = decrypt_password("not-valid-base64!@#");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short() {
        // Valid base64 but too short (< 12 bytes)
        let short_data = BASE64.encode(&[1, 2, 3]);
        let result = decrypt_password(&short_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid encrypted data"));
    }

    #[test]
    fn test_decrypt_tampered_data() {
        let original = "MyPassword";
        let mut encrypted = encrypt_password(original).unwrap();

        // Tamper with the encrypted data by adding characters
        encrypted.push('X');

        let result = decrypt_password(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_password() {
        let empty = "";
        let encrypted = encrypt_password(empty).unwrap();
        let decrypted = decrypt_password(&encrypted).unwrap();
        assert_eq!(decrypted, empty);
    }

    #[test]
    fn test_unicode_password() {
        let unicode = "パスワード🔐";
        let encrypted = encrypt_password(unicode).unwrap();
        let decrypted = decrypt_password(&encrypted).unwrap();
        assert_eq!(decrypted, unicode);
    }

    #[test]
    fn test_very_long_password() {
        let long_password = "a".repeat(1000);
        let encrypted = encrypt_password(&long_password).unwrap();
        let decrypted = decrypt_password(&encrypted).unwrap();
        assert_eq!(decrypted, long_password);
    }

    #[test]
    fn test_derive_key_consistency() {
        // Key derivation should be deterministic
        let key1 = derive_key();
        let key2 = derive_key();
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32); // AES-256 requires 32 bytes
    }

    #[test]
    fn test_special_characters() {
        let special = "p@ssw0rd!#$%^&*(){}[]|\\:;\"'<>,.?/~`";
        let encrypted = encrypt_password(special).unwrap();
        let decrypted = decrypt_password(&encrypted).unwrap();
        assert_eq!(decrypted, special);
    }
}
