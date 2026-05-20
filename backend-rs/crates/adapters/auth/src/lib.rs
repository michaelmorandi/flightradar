//! Auth adapter: Argon2id password hashing + HS256 JWT issue/verify.

pub mod hasher;
pub mod jwt;

pub use hasher::Argon2PasswordHasher;
pub use jwt::{JwtSigner, JwtTokenIssuer, JwtTokenVerifier};
