use mongodb::{
    Collection,
    bson::{Document, oid::ObjectId},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    ForgeError,
    db::{
        mongo_find::ODMFind,
        mongo_odm::{ODMongo, odm},
    },
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

    fn coll() -> Result<Collection<Self>, ForgeError> {
        Ok(odm()?.coll::<Self>(Self::coll_name()))
    }

    // == Hooks ===================================================

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

    // == Many Operations =========================================

    fn create_many(docs: &mut Vec<Self>) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.create_many::<Self>(docs).await }
    }

    fn update_many(
        filter: Document,
        update: Document,
    ) -> impl Future<Output = Result<mongodb::results::UpdateResult, ForgeError>> + Send {
        async move { odm()?.update_many::<Self>(filter, update).await }
    }

    // == CRUD ====================================================

    fn create(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.create::<Self>(self).await }
    }

    fn update(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.update::<Self>(self).await }
    }

    fn save(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.save::<Self>(self).await }
    }

    fn replace(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.replace::<Self>(self).await }
    }

    fn delete(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move { odm()?.delete::<Self>(self).await }
    }

    // == Getting previus =========================================

    fn update_getting_previous(&mut self) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.update_getting_previous::<Self>(self).await }
    }

    fn replace_getting_previous(&mut self) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.replace_getting_previous::<Self>(self).await }
    }

    fn delete_getting_previous(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.delete_getting_previous::<Self>(id).await }
    }

    // == Getters =================================================

    fn get_by_id(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.get_by_id::<Self>(id).await }
    }

    fn get_by_hex_id(hex_id: &str) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move { odm()?.get_by_hex_id::<Self>(hex_id).await }
    }

    fn get_all() -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send {
        async move { odm()?.get_all::<Self>().await }
    }

    fn find<U>(filter: Document) -> Result<ODMFind<Self, U>, ForgeError>
    where
        U: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        Ok(odm()?.find::<Self, U>(filter))
    }
}

pub trait ODModelCollection<T: ODModel> {
    fn create(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn update(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn save(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn replace(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send;
    fn delete(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send;
}

impl<T: ODModel> ODModelCollection<T> for Vec<T> {
    fn create(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            for m in self.iter_mut() {
                m.before_create()?;
            }
            odm()?.create_many(self).await?;
            for m in self.iter_mut() {
                m.after_create()?;
            }
            Ok(())
        }
    }

    fn update(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            // hacerlo con bulkWrite
            for m in self.iter_mut() {
                m.update().await?;
            }
            Ok(())
        }
    }

    // one to one
    fn save(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            for m in self.iter_mut() {
                m.save().await?;
            }
            Ok(())
        }
    }

    // one to one
    fn replace(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            for m in self.iter_mut() {
                m.replace().await?;
            }
            Ok(())
        }
    }

    // one to one
    fn delete(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            for m in self.iter_mut() {
                m.delete().await?;
            }
            Ok(())
        }
    }
}
