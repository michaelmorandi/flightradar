//! User ↔ BSON.

use bson::{doc, Document};

use flightradar_domain::{Role, User, UserId};

use super::flight::{read_datetime, read_opt_str, unix_ms};
use crate::error::CodecError;

pub fn user_to_document(user: &User, password_hash: Option<&str>) -> Document {
    let mut doc = doc! {
        "_id": user.id.as_str(),
        "email": &user.email,
        "role": role_to_str(user.role),
        "is_active": user.is_active,
        "created_at": bson::DateTime::from_millis(unix_ms(user.created_at)),
    };
    if let Some(name) = &user.display_name {
        doc.insert("display_name", name);
    }
    if let Some(t) = user.last_login {
        doc.insert("last_login", bson::DateTime::from_millis(unix_ms(t)));
    }
    if let Some(h) = password_hash {
        doc.insert("password_hash", h);
    }
    doc
}

pub fn document_to_user(doc: &Document) -> Result<User, CodecError> {
    let id = doc
        .get_str("_id")
        .map_err(|_| CodecError::MissingField("_id"))?
        .to_owned();
    let email = doc
        .get_str("email")
        .map_err(|_| CodecError::MissingField("email"))?
        .to_owned();
    let role_str = doc
        .get_str("role")
        .map_err(|_| CodecError::MissingField("role"))?;
    let role = role_from_str(role_str)
        .ok_or_else(|| CodecError::InvalidValue("role", role_str.to_string()))?;
    let is_active = doc.get_bool("is_active").unwrap_or(true);
    let created_at = read_datetime(doc, "created_at")?;
    let display_name = read_opt_str(doc, "display_name")?;
    let last_login = match doc.get_datetime("last_login") {
        Ok(_) => Some(read_datetime(doc, "last_login")?),
        Err(_) => None,
    };
    Ok(User {
        id: UserId::new(id),
        email,
        role,
        display_name,
        is_active,
        created_at,
        last_login,
    })
}

pub fn read_password_hash(doc: &Document) -> Result<Option<String>, CodecError> {
    read_opt_str(doc, "password_hash")
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

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    fn user() -> User {
        User {
            id: UserId::new("u-1"),
            email: "a@b".into(),
            role: Role::Admin,
            display_name: Some("Admin".into()),
            is_active: true,
            created_at: t(0),
            last_login: Some(t(60)),
        }
    }

    #[test]
    fn roundtrip_user_with_password_hash() {
        let original = user();
        let doc = user_to_document(&original, Some("hash"));
        let parsed = document_to_user(&doc).unwrap();
        assert_eq!(parsed, original);
        assert_eq!(read_password_hash(&doc).unwrap().as_deref(), Some("hash"));
    }

    #[test]
    fn omits_password_hash_when_none() {
        let doc = user_to_document(&user(), None);
        assert!(!doc.contains_key("password_hash"));
        assert!(read_password_hash(&doc).unwrap().is_none());
    }

    #[test]
    fn omits_last_login_when_none() {
        let mut u = user();
        u.last_login = None;
        let doc = user_to_document(&u, None);
        assert!(!doc.contains_key("last_login"));
        let parsed = document_to_user(&doc).unwrap();
        assert!(parsed.last_login.is_none());
    }

    #[test]
    fn each_role_string_maps_back() {
        for role in [Role::Anonymous, Role::User, Role::Admin] {
            let mut u = user();
            u.role = role;
            let doc = user_to_document(&u, None);
            assert_eq!(document_to_user(&doc).unwrap().role, role);
        }
    }

    #[test]
    fn unknown_role_returns_invalid_value() {
        let doc = doc! {
            "_id": "u-1",
            "email": "a@b",
            "role": "supreme-leader",
            "is_active": true,
            "created_at": bson::DateTime::now(),
        };
        let err = document_to_user(&doc).unwrap_err();
        assert!(matches!(err, CodecError::InvalidValue("role", _)));
    }

    #[test]
    fn missing_required_fields_caught() {
        let doc = doc! { "_id": "u-1" };
        assert!(matches!(
            document_to_user(&doc).unwrap_err(),
            CodecError::MissingField(_)
        ));
    }
}
