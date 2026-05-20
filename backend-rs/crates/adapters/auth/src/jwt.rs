//! HS256 JWT issue + verify. The signer holds an `EncodingKey` + matching
//! `DecodingKey`; we expose both ports so the server crate can wire them
//! into the auth service.

use std::sync::Arc;

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use flightradar_domain::ports::auth::{AuthError, TokenClaims, TokenIssuer, TokenVerifier};
use flightradar_domain::{Role, UserId};

#[derive(Clone)]
pub struct JwtSigner {
    encoding: Arc<EncodingKey>,
    decoding: Arc<DecodingKey>,
    /// Issuer claim. Bound at construction to avoid threading it through the
    /// trait surface; kept simple — single string, not a list.
    issuer: String,
}

impl std::fmt::Debug for JwtSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtSigner")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl JwtSigner {
    /// HS256-style signer from a shared secret. The secret must be at
    /// least 32 bytes for HS256 to be considered safe.
    pub fn from_secret(secret: &[u8], issuer: impl Into<String>) -> Result<Self, AuthError> {
        if secret.len() < 32 {
            return Err(AuthError::Backend(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "JWT secret must be at least 32 bytes",
            ))));
        }
        Ok(Self {
            encoding: Arc::new(EncodingKey::from_secret(secret)),
            decoding: Arc::new(DecodingKey::from_secret(secret)),
            issuer: issuer.into(),
        })
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }
}

#[derive(Debug, Clone)]
pub struct JwtTokenIssuer(JwtSigner);

impl JwtTokenIssuer {
    pub fn new(signer: JwtSigner) -> Self {
        Self(signer)
    }
}

#[derive(Debug, Clone)]
pub struct JwtTokenVerifier(JwtSigner);

impl JwtTokenVerifier {
    pub fn new(signer: JwtSigner) -> Self {
        Self(signer)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WireClaims {
    sub: String,
    role: String,
    iat: i64,
    exp: i64,
    iss: String,
}

impl WireClaims {
    fn from_domain(claims: &TokenClaims, issuer: &str) -> Self {
        Self {
            sub: claims.user_id.as_str().to_owned(),
            role: role_to_str(claims.role).to_owned(),
            iat: claims.issued_at.unix_timestamp(),
            exp: claims.expires_at.unix_timestamp(),
            iss: issuer.to_owned(),
        }
    }

    fn into_domain(self) -> Result<TokenClaims, AuthError> {
        let role = role_from_str(&self.role).ok_or(AuthError::Malformed)?;
        let issued_at =
            OffsetDateTime::from_unix_timestamp(self.iat).map_err(|_| AuthError::Malformed)?;
        let expires_at =
            OffsetDateTime::from_unix_timestamp(self.exp).map_err(|_| AuthError::Malformed)?;
        Ok(TokenClaims {
            user_id: UserId::new(self.sub),
            role,
            issued_at,
            expires_at,
        })
    }
}

impl TokenIssuer for JwtTokenIssuer {
    fn issue(&self, claims: &TokenClaims) -> Result<String, AuthError> {
        let wire = WireClaims::from_domain(claims, &self.0.issuer);
        encode(&Header::new(Algorithm::HS256), &wire, &self.0.encoding)
            .map_err(|e| AuthError::Backend(Box::new(e)))
    }
}

impl TokenVerifier for JwtTokenVerifier {
    fn verify(&self, token: &str) -> Result<TokenClaims, AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.0.issuer]);
        let data =
            decode::<WireClaims>(token, &self.0.decoding, &validation).map_err(|e| {
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::Expired,
                    _ => AuthError::Malformed,
                }
            })?;
        data.claims.into_domain()
    }
}

fn role_to_str(role: Role) -> &'static str {
    match role {
        Role::Anonymous => "anonymous",
        Role::User => "user",
        Role::Admin => "admin",
    }
}

fn role_from_str(s: &str) -> Option<Role> {
    match s {
        "anonymous" => Some(Role::Anonymous),
        "user" => Some(Role::User),
        "admin" => Some(Role::Admin),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn signer() -> JwtSigner {
        // 32-byte secret.
        JwtSigner::from_secret(&[0xAB; 32], "flightradar-test").unwrap()
    }

    fn claims(role: Role, ttl_secs: i64) -> TokenClaims {
        let now = OffsetDateTime::now_utc();
        TokenClaims {
            user_id: UserId::new("u-1"),
            role,
            issued_at: now,
            expires_at: now + Duration::seconds(ttl_secs),
        }
    }

    #[test]
    fn issue_then_verify_round_trips() {
        let s = signer();
        let issuer = JwtTokenIssuer::new(s.clone());
        let verifier = JwtTokenVerifier::new(s);
        let original = claims(Role::Admin, 60);
        let token = issuer.issue(&original).unwrap();
        let back = verifier.verify(&token).unwrap();
        assert_eq!(back.user_id, original.user_id);
        assert_eq!(back.role, original.role);
        assert_eq!(
            back.issued_at.unix_timestamp(),
            original.issued_at.unix_timestamp()
        );
        assert_eq!(
            back.expires_at.unix_timestamp(),
            original.expires_at.unix_timestamp()
        );
    }

    #[test]
    fn each_role_round_trips() {
        let s = signer();
        let issuer = JwtTokenIssuer::new(s.clone());
        let verifier = JwtTokenVerifier::new(s);
        for role in [Role::Anonymous, Role::User, Role::Admin] {
            let token = issuer.issue(&claims(role, 60)).unwrap();
            assert_eq!(verifier.verify(&token).unwrap().role, role);
        }
    }

    #[test]
    fn rejects_short_secret() {
        let err = JwtSigner::from_secret(&[0; 16], "x").unwrap_err();
        assert!(matches!(err, AuthError::Backend(_)));
    }

    #[test]
    fn rejects_token_signed_with_other_secret() {
        let a = JwtSigner::from_secret(&[0xAA; 32], "iss").unwrap();
        let b = JwtSigner::from_secret(&[0xBB; 32], "iss").unwrap();
        let token = JwtTokenIssuer::new(a)
            .issue(&claims(Role::Admin, 60))
            .unwrap();
        let err = JwtTokenVerifier::new(b).verify(&token).unwrap_err();
        assert!(matches!(err, AuthError::Malformed));
    }

    #[test]
    fn rejects_token_from_other_issuer() {
        let a = JwtSigner::from_secret(&[0xAB; 32], "issuer-A").unwrap();
        let b = JwtSigner::from_secret(&[0xAB; 32], "issuer-B").unwrap();
        let token = JwtTokenIssuer::new(a)
            .issue(&claims(Role::Admin, 60))
            .unwrap();
        let err = JwtTokenVerifier::new(b).verify(&token).unwrap_err();
        assert!(matches!(err, AuthError::Malformed));
    }

    #[test]
    fn rejects_expired_token() {
        let s = signer();
        let issuer = JwtTokenIssuer::new(s.clone());
        let verifier = JwtTokenVerifier::new(s);
        // ttl in the past
        let token = issuer.issue(&claims(Role::Admin, -120)).unwrap();
        let err = verifier.verify(&token).unwrap_err();
        assert!(matches!(err, AuthError::Expired));
    }

    #[test]
    fn rejects_malformed_token() {
        let v = JwtTokenVerifier::new(signer());
        assert!(matches!(
            v.verify("not.a.jwt").unwrap_err(),
            AuthError::Malformed
        ));
        assert!(matches!(v.verify("").unwrap_err(), AuthError::Malformed));
    }
}
