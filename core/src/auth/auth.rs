use mongodb::bson::oid::ObjectId;

use crate::{Context, ForgeError};

#[derive(Clone)]
pub struct Auth {
    pub id: String,
    pub user_id: ObjectId,
}

impl Auth {
    pub fn new() -> Self {
        Self {
            id: "".into(),
            user_id: ObjectId::new(),
        }
    }

    pub fn load(c: &mut Context) -> Result<Self, ForgeError> {
        let session_id = c.session_id.clone().ok_or_else(|| ForgeError::unauthorized())?;
        Ok(Self {
            id: session_id,
            user_id: ObjectId::new(),
        })
    }

    pub fn can(&self, _action: &str) -> Result<(), ForgeError> {
        Err(ForgeError::not_implemented())
    }
}
