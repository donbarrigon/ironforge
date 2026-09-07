use mongodb::Cursor;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{Document, doc, to_document};
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;
use std::str::FromStr;
use tokio_stream::StreamExt;

use crate::{ForgeError, db::mongo::coll};

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

    // == CRUD ===================================================
    fn save(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            match self.get_id() {
                Some(_) => self.update().await,
                None => self.create().await,
            }
        }
    }

    fn create(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            self.set_id(ObjectId::new());
            self.before_create()?;
            let _ = coll::<Self>(Self::coll_name())?.insert_one(&*self).await.map_err(|e| {
                let msg = format!("Failed to create document: {}", e);
                crate::log::error(&msg, None);
                ForgeError::internal().message(msg).caused_by(e)
            })?;
            // self.set_id(res.inserted_id.as_object_id().unwrap());
            self.after_create()?;
            Ok(())
        }
    }

    fn update(&mut self) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            self.before_update()?;
            let mut data = to_document(&*self).map_err(|e| {
                let msg = format!("Failed to serialize document: {}", e);
                crate::log::error(&msg, None);
                ForgeError::internal().message(msg).caused_by(e)
            })?;
            data.remove("_id");
            let id = self.get_id().ok_or_else(|| {
                let msg = "Document id is None";
                crate::log::error(msg, None);
                ForgeError::not_found().message(msg)
            })?;

            let res = coll::<Self>(Self::coll_name())?
                .update_one(doc! {"_id": id}, data)
                .await
                .map_err(|e| {
                    let msg = format!("Failed to update document: {}", e);
                    crate::log::error(&msg, None);
                    ForgeError::internal().message(msg).caused_by(e)
                })?;

            if res.matched_count == 0 {
                let msg = format!("Document [{}] not found", id);
                crate::log::error(&msg, None);
                return Err(ForgeError::not_found().message(msg));
            }
            if res.modified_count == 0 {
                return Ok(());
            }

            self.after_update()?;
            Ok(())
        }
    }

    fn replace(&mut self, id: ObjectId) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            self.set_id(id);
            self.before_replace()?;
            let res = coll::<Self>(Self::coll_name())?
                .replace_one(doc! {"_id": id}, &*self)
                .await
                .map_err(|e| {
                    let msg = format!("Failed to replace document: [{}]", id.to_hex());
                    crate::log::error(&msg, None);
                    ForgeError::internal().message(msg).caused_by(e)
                })?;

            if res.matched_count == 0 {
                let msg = format!("Document [{}] not found", id);
                crate::log::error(&msg, None);
                return Err(ForgeError::not_found().message(msg));
            }
            if res.modified_count == 0 {
                return Ok(());
            }

            self.after_replace()?;
            Ok(())
        }
    }

    fn replace_hex<'a>(
        &'a mut self,
        id: impl Into<String> + Send + 'a,
    ) -> impl Future<Output = Result<(), ForgeError>> + Send + 'a {
        async move {
            let hid = id.into();
            let oid = ObjectId::from_str(&hid).map_err(|e| {
                let msg = format!("Invalid ObjectId: [{}]", hid);
                crate::log::error(&msg, None);
                ForgeError::bad_request().message(msg).caused_by(e)
            })?;
            self.replace(oid).await
        }
    }

    fn replace_getting_previous(&mut self, id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        // async move {
        //     let prev = Self::get_by_id(id).await?;
        //     self.replace(id).await?;
        //     Ok(prev)
        // }
        async move {
            let res = coll::<Self>(Self::coll_name())?
                .find_one_and_replace(doc! {"_id": id}, &*self)
                .await
                .map_err(|e| {
                    let msg = format!("Failed to replace document: [{}]", id);
                    crate::log::error(&msg, None);
                    ForgeError::internal().message(msg).caused_by(e)
                })?;
            match res {
                Some(data) => Ok(data),
                None => {
                    let msg = format!("Document [{}] not found", id);
                    crate::log::error(&msg, None);
                    Err(ForgeError::not_found().message(msg))
                }
            }
        }
    }

    fn replace_getting_previous_hex<'a>(
        &'a mut self,
        id: impl Into<String> + Send + 'a,
    ) -> impl Future<Output = Result<Self, ForgeError>> + Send + 'a {
        async move {
            let hid = id.into();
            let oid = ObjectId::from_str(&hid).map_err(|e| {
                let msg = format!("Invalid ObjectId: [{}]", hid);
                crate::log::error(&msg, None);
                ForgeError::bad_request().message(msg).caused_by(e)
            })?;
            self.replace_getting_previous(oid).await
        }
    }

    fn delete(id: ObjectId) -> impl Future<Output = Result<(), ForgeError>> + Send {
        async move {
            let res = coll::<Self>(Self::coll_name())?
                .delete_one(doc! {"_id": id})
                .await
                .map_err(|e| {
                    let msg = format!("Failed to delete document: [{}]", id);
                    crate::log::error(&msg, None);
                    ForgeError::internal().message(msg).caused_by(e)
                })?;
            if res.deleted_count == 0 {
                let msg = format!("Document [{}] not found", id);
                crate::log::error(&msg, None);
                return Err(ForgeError::not_found().message(msg));
            }
            Ok(())
        }
    }

    fn delete_hex<'a>(id: impl Into<String> + Send + 'a) -> impl Future<Output = Result<(), ForgeError>> + Send + 'a {
        async move {
            let hid = id.into();
            let oid = ObjectId::from_str(&hid).map_err(|e| {
                let msg = format!("Invalid ObjectId: [{}]", hid);
                crate::log::error(&msg, None);
                ForgeError::bad_request().message(msg).caused_by(e)
            })?;
            Self::delete(oid).await
        }
    }

    fn delete_getting_previous(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        // async move {
        //     let prev = Self::get_by_id(id).await?;
        //     Self::delete(id).await?;
        //     Ok(prev)
        // }
        async move {
            let res = coll::<Self>(Self::coll_name())?
                .find_one_and_delete(doc! {"_id": id})
                .await
                .map_err(|e| {
                    let msg = format!("Failed to delete document: [{}]", id);
                    crate::log::error(&msg, None);
                    ForgeError::internal().message(msg).caused_by(e)
                })?;
            match res {
                Some(data) => Ok(data),
                None => {
                    let msg = format!("Document [{}] not found", id);
                    crate::log::error(&msg, None);
                    Err(ForgeError::not_found().message(msg))
                }
            }
        }
    }

    fn delete_getting_previous_hex<'a>(
        id: impl Into<String> + Send + 'a,
    ) -> impl Future<Output = Result<Self, ForgeError>> + Send + 'a {
        async move {
            let hid = id.into();
            let oid = ObjectId::from_str(&hid).map_err(|e| {
                let msg = format!("Invalid ObjectId: [{}]", hid);
                crate::log::error(&msg, None);
                ForgeError::bad_request().message(msg).caused_by(e)
            })?;
            Self::delete_getting_previous(oid).await
        }
    }

    // == get_one ================================================
    fn get_by_id(id: ObjectId) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move {
            let data = coll::<Self>(Self::coll_name())?
                .find_one(doc! {"_id": id})
                .await
                .map_err(|e| {
                    let msg = format!("Failed to get document: [{}]", id);
                    crate::log::error(&msg, None);
                    ForgeError::internal().message(msg).caused_by(e)
                })?;
            match data {
                Some(data) => Ok(data),
                None => {
                    let msg = format!("Document [{}] not found", id);
                    crate::log::error(&msg, None);
                    Err(ForgeError::not_found().message(msg))
                }
            }
        }
    }
    fn get_by_hex_id<'a>(
        id: impl Into<String> + Send + 'a,
    ) -> impl Future<Output = Result<Self, ForgeError>> + Send + 'a {
        async move {
            let hid = id.into();
            let oid = ObjectId::from_str(&hid).map_err(|e| {
                let msg = format!("Invalid ObjectId: [{}]", hid);
                crate::log::error(&msg, None);
                ForgeError::bad_request().message(msg).caused_by(e)
            })?;
            Self::get_by_id(oid).await
        }
    }

    fn get_one(filter: Document) -> impl Future<Output = Result<Self, ForgeError>> + Send {
        async move {
            let data = coll::<Self>(Self::coll_name())?.find_one(filter).await.map_err(|e| {
                let msg = format!("Failed to get document");
                crate::log::error(&msg, None);
                ForgeError::internal().message(msg).caused_by(e)
            })?;
            match data {
                Some(data) => Ok(data),
                None => {
                    let msg = "Document not found";
                    crate::log::error(msg, None);
                    Err(ForgeError::not_found().message(msg))
                }
            }
        }
    }

    // == GetAll =================================================
    fn get_all(filter: Document) -> impl Future<Output = Result<Vec<Self>, ForgeError>> + Send {
        async move {
            let mut cursor = Self::find(filter).await?;
            let mut docs: Vec<Self> = Vec::new();
            while let Some(doc) = cursor.try_next().await.map_err(|e| {
                let msg = format!("Failed to iterate over documents");
                crate::log::error(&msg, None);
                ForgeError::internal().message(msg).caused_by(e)
            })? {
                docs.push(doc);
            }
            Ok(docs)
        }
    }

    // == Find ===================================================

    fn find(filter: Document) -> impl Future<Output = Result<Cursor<Self>, ForgeError>> + Send {
        async move {
            coll::<Self>(Self::coll_name())?.find(filter).await.map_err(|e| {
                let msg = format!("Failed to find documents");
                crate::log::error(&msg, None);
                ForgeError::internal().message(msg).caused_by(e)
            })
        }
    }

    // == agregate ===============================================
    //TODO: agregate methods
}
