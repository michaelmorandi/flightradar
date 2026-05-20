//! Authentication use cases: anonymous login, admin login, token verify.

use std::sync::Arc;

use time::{Duration, OffsetDateTime};
use tracing::warn;

use flightradar_domain::ports::auth::{PasswordHasher, TokenClaims, TokenIssuer, TokenVerifier};
use flightradar_domain::ports::clock::Clock;
use flightradar_domain::ports::repositories::{RepositoryError, UserRepository};
use flightradar_domain::{Role, User, UserId};

use crate::error::ApplicationError;

#[derive(Debug, Clone, Copy)]
pub struct AuthServiceConfig {
    pub token_lifetime: Duration,
    pub anonymous_email: &'static str,
}

impl Default for AuthServiceConfig {
    fn default() -> Self {
        Self {
            token_lifetime: Duration::minutes(15),
            anonymous_email: "anonymous@flightradar.local",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoginOutcome {
    pub user: User,
    pub token: String,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct AuthService {
    users: Arc<dyn UserRepository>,
    hasher: Arc<dyn PasswordHasher>,
    issuer: Arc<dyn TokenIssuer>,
    verifier: Arc<dyn TokenVerifier>,
    clock: Arc<dyn Clock>,
    config: AuthServiceConfig,
}

impl AuthService {
    pub fn new(
        users: Arc<dyn UserRepository>,
        hasher: Arc<dyn PasswordHasher>,
        issuer: Arc<dyn TokenIssuer>,
        verifier: Arc<dyn TokenVerifier>,
        clock: Arc<dyn Clock>,
        config: AuthServiceConfig,
    ) -> Self {
        Self {
            users,
            hasher,
            issuer,
            verifier,
            clock,
            config,
        }
    }

    /// Issue a token for the anonymous user, creating the user record
    /// lazily if it does not exist yet.
    pub async fn anonymous_login(&self) -> Result<LoginOutcome, ApplicationError> {
        let email = self.config.anonymous_email;
        let user = if let Some(u) = self.users.find_by_email(email).await? {
            u
        } else {
            let new = User {
                id: UserId::new(format!("anon-{}", self.clock.now().unix_timestamp())),
                email: email.into(),
                role: Role::Anonymous,
                display_name: Some("Anonymous".into()),
                is_active: true,
                created_at: self.clock.now(),
                last_login: None,
            };
            self.users.upsert(&new, None).await?;
            new
        };

        self.touch_login(&user.id).await;
        let outcome = self.issue_token(user)?;
        Ok(outcome)
    }

    /// Verify email+password and issue a token. Returns `Unauthenticated`
    /// on any failure mode — no detail about *why* leaks to the caller.
    pub async fn admin_login(
        &self,
        email: &str,
        password: &str,
    ) -> Result<LoginOutcome, ApplicationError> {
        let user = self
            .users
            .find_by_email(email)
            .await?
            .ok_or(ApplicationError::Unauthenticated)?;

        if !user.is_active || user.role != Role::Admin {
            return Err(ApplicationError::Unauthenticated);
        }

        let hash = self
            .users
            .read_password_hash(&user.id)
            .await?
            .ok_or(ApplicationError::Unauthenticated)?;

        let ok = self.hasher.verify(password, &hash).await?;
        if !ok {
            return Err(ApplicationError::Unauthenticated);
        }

        self.touch_login(&user.id).await;
        self.issue_token(user)
    }

    pub fn verify_token(&self, token: &str) -> Result<TokenClaims, ApplicationError> {
        Ok(self.verifier.verify(token)?)
    }

    fn issue_token(&self, user: User) -> Result<LoginOutcome, ApplicationError> {
        let now = self.clock.now();
        let expires_at = now + self.config.token_lifetime;
        let claims = TokenClaims {
            user_id: user.id.clone(),
            role: user.role,
            issued_at: now,
            expires_at,
        };
        let token = self.issuer.issue(&claims)?;
        Ok(LoginOutcome {
            user,
            token,
            expires_at,
        })
    }

    async fn touch_login(&self, id: &UserId) {
        if let Err(err) = self.users.touch_last_login(id, self.clock.now()).await {
            // Non-fatal: log and continue, do not block the login.
            if !matches!(err, RepositoryError::NotFound) {
                warn!(error = %err, user = %id.as_str(), "failed to touch last_login");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;

    use flightradar_domain::ports::auth::{AuthError, TokenClaims};
    use flightradar_domain::ports::repositories::RepoResult;

    use super::*;

    #[derive(Debug)]
    struct FixedClock(OffsetDateTime);
    impl Clock for FixedClock {
        fn now(&self) -> OffsetDateTime {
            self.0
        }
    }
    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    // -- User repo ------------------------------------------------------

    #[derive(Debug, Default)]
    struct InMemUserRepo {
        by_email: StdMutex<std::collections::HashMap<String, (User, Option<String>)>>,
        last_logins: StdMutex<Vec<(UserId, OffsetDateTime)>>,
    }

    impl InMemUserRepo {
        fn seed(&self, user: User, hash: Option<String>) {
            self.by_email
                .lock()
                .unwrap()
                .insert(user.email.clone(), (user, hash));
        }
    }

    #[async_trait]
    impl UserRepository for InMemUserRepo {
        async fn find_by_id(&self, _id: &UserId) -> RepoResult<Option<User>> {
            unimplemented!()
        }
        async fn find_by_email(&self, email: &str) -> RepoResult<Option<User>> {
            Ok(self
                .by_email
                .lock()
                .unwrap()
                .get(email)
                .map(|(u, _)| u.clone()))
        }
        async fn upsert(&self, user: &User, hash: Option<&str>) -> RepoResult<()> {
            self.by_email
                .lock()
                .unwrap()
                .insert(user.email.clone(), (user.clone(), hash.map(str::to_owned)));
            Ok(())
        }
        async fn read_password_hash(&self, id: &UserId) -> RepoResult<Option<String>> {
            for (_, (u, h)) in self.by_email.lock().unwrap().iter() {
                if u.id == *id {
                    return Ok(h.clone());
                }
            }
            Ok(None)
        }
        async fn touch_last_login(&self, id: &UserId, when: OffsetDateTime) -> RepoResult<()> {
            self.last_logins.lock().unwrap().push((id.clone(), when));
            Ok(())
        }
    }

    // -- Crypto stubs ---------------------------------------------------

    #[derive(Debug)]
    struct PlainHasher; // "verify" iff hash == plaintext

    #[async_trait]
    impl PasswordHasher for PlainHasher {
        async fn hash(&self, plaintext: &str) -> Result<String, AuthError> {
            Ok(plaintext.into())
        }
        async fn verify(&self, plaintext: &str, hash: &str) -> Result<bool, AuthError> {
            Ok(plaintext == hash)
        }
    }

    #[derive(Debug)]
    struct DummyIssuer;
    impl TokenIssuer for DummyIssuer {
        fn issue(&self, claims: &TokenClaims) -> Result<String, AuthError> {
            Ok(format!(
                "TOKEN({},{:?},{})",
                claims.user_id.as_str(),
                claims.role,
                claims.expires_at.unix_timestamp()
            ))
        }
    }

    #[derive(Debug)]
    struct DummyVerifier;
    impl TokenVerifier for DummyVerifier {
        fn verify(&self, _token: &str) -> Result<TokenClaims, AuthError> {
            Err(AuthError::Malformed)
        }
    }

    fn admin(email: &str) -> User {
        User {
            id: UserId::new("user-1"),
            email: email.into(),
            role: Role::Admin,
            display_name: None,
            is_active: true,
            created_at: t(0),
            last_login: None,
        }
    }

    fn build(users: Arc<InMemUserRepo>, clock_t: OffsetDateTime) -> AuthService {
        AuthService::new(
            users,
            Arc::new(PlainHasher),
            Arc::new(DummyIssuer),
            Arc::new(DummyVerifier),
            Arc::new(FixedClock(clock_t)),
            AuthServiceConfig::default(),
        )
    }

    // -- Tests ----------------------------------------------------------

    #[tokio::test]
    async fn anonymous_login_creates_user_on_first_call() {
        let users = Arc::new(InMemUserRepo::default());
        let svc = build(users.clone(), t(0));

        let out = svc.anonymous_login().await.unwrap();
        assert_eq!(out.user.role, Role::Anonymous);
        assert!(out.token.starts_with("TOKEN("));
        assert_eq!(out.expires_at, t(0) + Duration::minutes(15));
        assert_eq!(users.by_email.lock().unwrap().len(), 1);
        assert_eq!(users.last_logins.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn anonymous_login_reuses_existing_user() {
        let users = Arc::new(InMemUserRepo::default());
        let svc = build(users.clone(), t(0));

        let first = svc.anonymous_login().await.unwrap();
        let second = svc.anonymous_login().await.unwrap();
        assert_eq!(first.user.id, second.user.id);
        assert_eq!(users.by_email.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn admin_login_succeeds_on_match() {
        let users = Arc::new(InMemUserRepo::default());
        users.seed(admin("a@b"), Some("hunter2".into()));
        let svc = build(users, t(0));

        let out = svc.admin_login("a@b", "hunter2").await.unwrap();
        assert_eq!(out.user.role, Role::Admin);
    }

    #[tokio::test]
    async fn admin_login_rejects_unknown_email() {
        let users = Arc::new(InMemUserRepo::default());
        let svc = build(users, t(0));
        let err = svc.admin_login("nobody@b", "x").await.unwrap_err();
        assert!(matches!(err, ApplicationError::Unauthenticated));
    }

    #[tokio::test]
    async fn admin_login_rejects_wrong_password() {
        let users = Arc::new(InMemUserRepo::default());
        users.seed(admin("a@b"), Some("right".into()));
        let svc = build(users, t(0));
        let err = svc.admin_login("a@b", "wrong").await.unwrap_err();
        assert!(matches!(err, ApplicationError::Unauthenticated));
    }

    #[tokio::test]
    async fn admin_login_rejects_inactive_user() {
        let users = Arc::new(InMemUserRepo::default());
        let mut u = admin("a@b");
        u.is_active = false;
        users.seed(u, Some("pw".into()));
        let svc = build(users, t(0));
        let err = svc.admin_login("a@b", "pw").await.unwrap_err();
        assert!(matches!(err, ApplicationError::Unauthenticated));
    }

    #[tokio::test]
    async fn admin_login_rejects_non_admin_role() {
        let users = Arc::new(InMemUserRepo::default());
        let mut u = admin("a@b");
        u.role = Role::User;
        users.seed(u, Some("pw".into()));
        let svc = build(users, t(0));
        let err = svc.admin_login("a@b", "pw").await.unwrap_err();
        assert!(matches!(err, ApplicationError::Unauthenticated));
    }

    #[tokio::test]
    async fn admin_login_rejects_user_with_no_password_hash() {
        let users = Arc::new(InMemUserRepo::default());
        users.seed(admin("a@b"), None);
        let svc = build(users, t(0));
        let err = svc.admin_login("a@b", "anything").await.unwrap_err();
        assert!(matches!(err, ApplicationError::Unauthenticated));
    }

    #[tokio::test]
    async fn verify_token_delegates_to_verifier() {
        let users = Arc::new(InMemUserRepo::default());
        let svc = build(users, t(0));
        let err = svc.verify_token("anything").unwrap_err();
        assert!(matches!(err, ApplicationError::Auth(AuthError::Malformed)));
    }
}
