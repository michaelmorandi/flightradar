use async_trait::async_trait;
use bson::{doc, Document};
use mongodb::options::ReplaceOptions;
use mongodb::{Collection, Database};
use time::OffsetDateTime;

use flightradar_domain::ports::repositories::{RepoResult, UserRepository};
use flightradar_domain::{User, UserId};

use crate::codec::flight::unix_ms;
use crate::codec::user::{document_to_user, read_password_hash, user_to_document};
use crate::collections::USERS;
use crate::error::map_mongo_error;

#[derive(Debug, Clone)]
pub struct MongoUserRepository {
    col: Collection<Document>,
}

impl MongoUserRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            col: db.collection(USERS),
        }
    }
}

#[async_trait]
impl UserRepository for MongoUserRepository {
    async fn find_by_id(&self, id: &UserId) -> RepoResult<Option<User>> {
        let found = self
            .col
            .find_one(doc! { "_id": id.as_str() })
            .await
            .map_err(map_mongo_error)?;
        match found {
            Some(d) => Ok(Some(document_to_user(&d)?)),
            None => Ok(None),
        }
    }

    async fn find_by_email(&self, email: &str) -> RepoResult<Option<User>> {
        let found = self
            .col
            .find_one(doc! { "email": email })
            .await
            .map_err(map_mongo_error)?;
        match found {
            Some(d) => Ok(Some(document_to_user(&d)?)),
            None => Ok(None),
        }
    }

    async fn upsert(&self, user: &User, hashed_password: Option<&str>) -> RepoResult<()> {
        let doc = user_to_document(user, hashed_password);
        self.col
            .replace_one(doc! { "_id": user.id.as_str() }, doc)
            .with_options(ReplaceOptions::builder().upsert(true).build())
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }

    async fn read_password_hash(&self, id: &UserId) -> RepoResult<Option<String>> {
        let found = self
            .col
            .find_one(doc! { "_id": id.as_str() })
            .await
            .map_err(map_mongo_error)?;
        match found {
            Some(d) => Ok(read_password_hash(&d)?),
            None => Ok(None),
        }
    }

    async fn touch_last_login(&self, id: &UserId, when: OffsetDateTime) -> RepoResult<()> {
        self.col
            .update_one(
                doc! { "_id": id.as_str() },
                doc! { "$set": { "last_login": bson::DateTime::from_millis(unix_ms(when)) } },
            )
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }
}
