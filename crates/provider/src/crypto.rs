use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use thiserror::Error;

const NONCE_LENGTH: usize = 12;

#[derive(Clone, Debug)]
pub struct EncryptedCredential {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_version: i32,
}

#[derive(Clone)]
pub struct EnvelopeCipher {
    key: [u8; 32],
    key_version: i32,
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("credential master key must be base64 encoded 32 bytes")]
    InvalidKey,
    #[error("credential encryption failed")]
    Encryption,
    #[error("credential decryption failed")]
    Decryption,
    #[error("decrypted credential is not UTF-8")]
    InvalidPlaintext,
}

impl EnvelopeCipher {
    /// Builds a cipher from a base64 encoded AES-256 key.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError::InvalidKey`] unless the decoded key is exactly 32 bytes.
    pub fn from_base64(value: &str, key_version: i32) -> Result<Self, CryptoError> {
        let decoded = STANDARD
            .decode(value)
            .map_err(|_| CryptoError::InvalidKey)?;
        let key = decoded.try_into().map_err(|_| CryptoError::InvalidKey)?;
        Ok(Self { key, key_version })
    }

    /// Encrypts a credential using a fresh random nonce.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError`] if cipher initialization or authenticated encryption fails.
    pub fn encrypt(&self, plaintext: &str) -> Result<EncryptedCredential, CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| CryptoError::InvalidKey)?;
        let mut nonce = [0_u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce);
        let nonce_value = Nonce::from(nonce);
        let ciphertext = cipher
            .encrypt(&nonce_value, plaintext.as_bytes())
            .map_err(|_| CryptoError::Encryption)?;
        Ok(EncryptedCredential {
            ciphertext,
            nonce: nonce.to_vec(),
            key_version: self.key_version,
        })
    }

    /// Decrypts a credential created by the active key version.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError`] for a key-version mismatch, invalid nonce, failed authentication,
    /// or non-UTF-8 plaintext.
    pub fn decrypt(&self, encrypted: &EncryptedCredential) -> Result<String, CryptoError> {
        if encrypted.key_version != self.key_version || encrypted.nonce.len() != NONCE_LENGTH {
            return Err(CryptoError::Decryption);
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| CryptoError::InvalidKey)?;
        let nonce: [u8; NONCE_LENGTH] = encrypted
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| CryptoError::Decryption)?;
        let plaintext = cipher
            .decrypt(&Nonce::from(nonce), encrypted.ciphertext.as_ref())
            .map_err(|_| CryptoError::Decryption)?;
        String::from_utf8(plaintext).map_err(|_| CryptoError::InvalidPlaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_round_trip_and_random_nonce() {
        let key = STANDARD.encode([7_u8; 32]);
        let cipher = EnvelopeCipher::from_base64(&key, 3).expect("valid key");
        let first = cipher.encrypt("secret-token").expect("encrypted");
        let second = cipher.encrypt("secret-token").expect("encrypted");
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
        assert_eq!(cipher.decrypt(&first).expect("decrypted"), "secret-token");
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        let cipher = EnvelopeCipher::from_base64(&STANDARD.encode([7_u8; 32]), 1).unwrap();
        let other = EnvelopeCipher::from_base64(&STANDARD.encode([8_u8; 32]), 1).unwrap();
        let encrypted = cipher.encrypt("secret-token").unwrap();
        assert!(other.decrypt(&encrypted).is_err());
    }
}
