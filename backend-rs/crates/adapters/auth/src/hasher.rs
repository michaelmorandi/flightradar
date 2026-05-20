//! Argon2id password hasher.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher as _, PasswordVerifier as _};
use async_trait::async_trait;

use flightradar_domain::ports::auth::{AuthError, PasswordHasher};

/// Argon2id hasher with sensible defaults (m=19 MiB, t=2, p=1) per OWASP.
#[derive(Debug, Default, Clone)]
pub struct Argon2PasswordHasher;

#[async_trait]
impl PasswordHasher for Argon2PasswordHasher {
    async fn hash(&self, plaintext: &str) -> Result<String, AuthError> {
        let plaintext = plaintext.to_owned();
        let join = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let salt = SaltString::generate(&mut OsRng);
            let argon = Argon2::default();
            argon
                .hash_password(plaintext.as_bytes(), &salt)
                .map(|h| h.to_string())
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| AuthError::Backend(Box::new(e)))?;
        join.map_err(|msg| AuthError::Backend(Box::new(std::io::Error::other(msg))))
    }

    async fn verify(&self, plaintext: &str, hash: &str) -> Result<bool, AuthError> {
        let plaintext = plaintext.to_owned();
        let hash = hash.to_owned();
        let res = tokio::task::spawn_blocking(move || {
            let Ok(parsed) = PasswordHash::new(&hash) else {
                return Ok(false);
            };
            let argon = Argon2::default();
            match argon.verify_password(plaintext.as_bytes(), &parsed) {
                Ok(()) => Ok(true),
                Err(argon2::password_hash::Error::Password) => Ok(false),
                Err(e) => Err(e.to_string()),
            }
        })
        .await
        .map_err(|e| AuthError::Backend(Box::new(e)))?;
        res.map_err(|msg| AuthError::Backend(Box::new(std::io::Error::other(msg))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hash_then_verify_succeeds() {
        let h = Argon2PasswordHasher;
        let hash = h.hash("hunter2").await.unwrap();
        assert!(hash.starts_with("$argon2"));
        assert!(h.verify("hunter2", &hash).await.unwrap());
    }

    #[tokio::test]
    async fn verify_wrong_password_returns_false() {
        let h = Argon2PasswordHasher;
        let hash = h.hash("hunter2").await.unwrap();
        assert!(!h.verify("wrong", &hash).await.unwrap());
    }

    #[tokio::test]
    async fn hash_is_salted_so_two_hashes_differ() {
        let h = Argon2PasswordHasher;
        let h1 = h.hash("hunter2").await.unwrap();
        let h2 = h.hash("hunter2").await.unwrap();
        assert_ne!(h1, h2);
        // …but both verify.
        assert!(h.verify("hunter2", &h1).await.unwrap());
        assert!(h.verify("hunter2", &h2).await.unwrap());
    }

    #[tokio::test]
    async fn verify_garbage_hash_returns_false_not_error() {
        let h = Argon2PasswordHasher;
        assert!(!h.verify("x", "not-a-real-hash").await.unwrap());
    }

    #[tokio::test]
    async fn empty_password_round_trips() {
        let h = Argon2PasswordHasher;
        let hash = h.hash("").await.unwrap();
        assert!(h.verify("", &hash).await.unwrap());
        assert!(!h.verify("nonempty", &hash).await.unwrap());
    }
}
