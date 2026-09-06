use mongodb::bson::oid::ObjectId;
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;

use crate::ForgeError;

pub trait MongoModel: Serialize + DeserializeOwned + Send + Sync + Clone + 'static {
    // == Manual methods =========================================
    fn coll_name() -> &'static str;
    fn get_id(&self) -> Option<ObjectId>;
    fn set_id(&mut self, id: ObjectId);

    // == Hooks ==================================================
    fn before_create(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn before_update(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn before_delete(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn after_create(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn after_update(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn after_delete(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }

    // == CRUD ===================================================
    fn save(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn create(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send;

    fn update(&mut self, id: ObjectId) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn update_hex(&mut self, id: impl Into<String>) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn update_getting_previous(&mut self, id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send;
    fn update_getting_previous_hex(
        &mut self,
        id: impl Into<String>,
    ) -> impl Future<Output = Result<Self, ForgeError>> + Send;

    fn replace(&mut self, id: ObjectId) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn replace_hex(&mut self, id: impl Into<String>) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn replace_getting_previous(&mut self, id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send;
    fn replace_getting_previous_hex(
        &mut self,
        id: impl Into<String>,
    ) -> impl Future<Output = Result<Self, ForgeError>> + Send;

    fn delete(id: ObjectId) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn delete_hex(id: impl Into<String>) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn delete_getting_previous(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send;
    fn delete_getting_previous_hex(id: impl Into<String>) -> impl Future<Output = Result<Self, ForgeError>> + Send;

    // == get_one ================================================
    fn get_by_id(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send;
    fn get_by_hex_id(id: impl Into<String>) -> impl Future<Output = Result<Self, ForgeError>> + Send;
    fn get_one(filter: impl Serialize) -> impl Future<Output = Result<Self, ForgeError>> + Send;

    // == get_all ================================================
    fn get_all() -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;
    fn get_all_paginated(page: u64, per_page: u64) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;
    fn get_all_cursor(cursor: ObjectId) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;
    fn get_all_cursor_hex(cursor: impl Into<String>) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;

    // == get_many ===============================================
    fn get_many(filter: impl Serialize) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;
    fn get_many_paginated(
        filter: impl Serialize,
        page: u64,
        per_page: u64,
    ) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;
    fn get_many_cursor(
        filter: impl Serialize,
        cursor: ObjectId,
    ) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;
    fn get_many_cursor_hex(
        filter: impl Serialize,
        cursor: impl Into<String>,
    ) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send;

    // == agregate ================================================
    //TODO: agregate methods
}
