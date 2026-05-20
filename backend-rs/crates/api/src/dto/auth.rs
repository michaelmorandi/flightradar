//! Auth DTOs.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use flightradar_domain::{Role, User};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: UserDto,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub role: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
}

impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id.as_str().to_owned(),
            email: u.email,
            role: role_str(u.role).to_owned(),
            display_name: u.display_name,
            is_admin: u.role == Role::Admin,
        }
    }
}

pub fn role_str(role: Role) -> &'static str {
    match role {
        Role::Anonymous => "anonymous",
        Role::User => "user",
        Role::Admin => "admin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flightradar_domain::UserId;

    fn user(role: Role) -> User {
        User {
            id: UserId::new("u-1"),
            email: "a@b".into(),
            role,
            display_name: Some("A".into()),
            is_active: true,
            created_at: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            last_login: None,
        }
    }

    #[test]
    fn user_dto_marks_admin_correctly() {
        let dto: UserDto = user(Role::Admin).into();
        assert!(dto.is_admin);
        assert_eq!(dto.role, "admin");

        let dto: UserDto = user(Role::User).into();
        assert!(!dto.is_admin);
        assert_eq!(dto.role, "user");

        let dto: UserDto = user(Role::Anonymous).into();
        assert!(!dto.is_admin);
        assert_eq!(dto.role, "anonymous");
    }
}
