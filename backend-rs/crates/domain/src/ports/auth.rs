//! Authentication ports — keep crypto and JWT details out of the domain.

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::entities::user::{Role, UserId};

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("token expired")]
    Expired,

    #[error("token malformed")]
    Malformed,

    #[error("auth backend error: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone)]
pub struct TokenClaims {
    pub user_id: UserId,
    pub role: Role,
    pub issued_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[async_trait]
pub trait PasswordHasher: Send + Sync + std::fmt::Debug {
    async fn hash(&self, plaintext: &str) -> Result<String, AuthError>;
    async fn verify(&self, plaintext: &str, hash: &str) -> Result<bool, AuthError>;
}

pub trait TokenIssuer: Send + Sync + std::fmt::Debug {
    fn issue(&self, claims: &TokenClaims) -> Result<String, AuthError>;
}

pub trait TokenVerifier: Send + Sync + std::fmt::Debug {
    fn verify(&self, token: &str) -> Result<TokenClaims, AuthError>;
}
