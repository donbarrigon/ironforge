use mongodb::bson::{Document, oid::ObjectId};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    ForgeError,
    db::mongo_odm::{ODMongo, odm},
};

pub trait ODModel: Serialize + DeserializeOwned + Send + Sync + Clone + 'static {
    fn coll_name() -> &'static str;
    fn get_id(&self) -> Option<ObjectId>;
    fn set_id(&mut self, id: ObjectId);
    fn set_hex_id(&mut self, hex_id: &str) -> Result<(), ForgeError> {
        let id = ODMongo::hex_to_object_id(hex_id)?;
        self.set_id(id);
        Ok(())
    }

    fn before_create(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn before_update(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn before_replace(&mut self) -> Result<(), ForgeError> {
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
    fn after_replace(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }
    fn after_delete(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }

    fn create(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.create(self).await }
    }

    fn update(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.update(self).await }
    }

    fn save(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.save(self).await }
    }

    fn replace(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.replace(self).await }
    }

    fn delete(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.delete(self).await }
    }

    fn update_getting_previous(&mut self) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.update_getting_previous(self).await }
    }

    fn replace_getting_previous(&mut self) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.replace_getting_previous(self).await }
    }

    fn delete_getting_previous(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.delete_getting_previous(id).await }
    }

    fn get_by_id(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.get_by_id(id).await }
    }

    fn get_by_hex_id(hex_id: &str) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.get_by_hex_id(hex_id).await }
    }

    fn get_one(filter: Document) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.get_one(filter).await }
    }

    fn get_all() -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send {
        async move { odm()?.get_all().await }
    }
}
