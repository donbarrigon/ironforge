use mongodb::bson::{Document, doc, to_document};
use mongodb::results::UpdateResult;
use mongodb::{Client, Collection, Database, bson::oid::ObjectId};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::str::FromStr;
use std::sync::OnceLock;
use tokio_stream::StreamExt;

use crate::db::mongo_find::ODMFind;
use crate::db::mongo_model::ODModel;
use crate::{ForgeError, env, log};

static ODMONGO: OnceLock<ODMongo> = OnceLock::new();

#[derive(Clone)]
pub struct ODMongo {
    pub db_name: String,
    pub client: Client,
}

impl ODMongo {
    /// Initializes the ODMongo instance with the provided database name and connection string from the environment.
    pub async fn init() -> Result<(), ForgeError> {
        let uri = env().db.string_connection.clone();
        let db_name = env().db.name.clone();

        let client = Client::with_uri_str(&uri).await.map_err(|e| {
            let msg = format!("Failed to connect/ping db: {}", e);
            log::error(&msg, None);
            ForgeError::internal().message(msg).caused_by(e)
        })?;

        let _ = client.list_database_names().await.map_err(|e| {
            let msg = format!("Failed to connect/ping db: {}", e);
            log::error(&msg, None);

            ForgeError::internal().message(msg).caused_by(e)
        })?;

        let odm = ODMongo { db_name, client };
        let _ = ODMONGO.set(odm);

        log::info("MongoDB ODM initialized successfully", None);
        Ok(())
    }

    pub async fn connect(db_name: impl Into<String>, uri: &str) -> Result<ODMongo, ForgeError> {
        let client = Client::with_uri_str(uri).await.map_err(|e| {
            let msg = format!("Failed to connect/ping db: {}", e);
            log::error(&msg, None);
            ForgeError::internal().message(msg).caused_by(e)
        })?;

        let _ = client.list_database_names().await.map_err(|e| {
            let msg = format!("Failed to connect/ping db: {}", e);
            log::error(&msg, None);

            ForgeError::internal().message(msg).caused_by(e)
        })?;

        Ok(ODMongo {
            db_name: db_name.into(),
            client,
        })
    }

    pub fn db(&self) -> Database {
        self.client.database(&self.db_name)
    }

    pub fn coll<T: Send + Sync>(&self, name: &str) -> Collection<T> {
        self.db().collection::<T>(name)
    }

    // ============================================================
    // CRUD OPERATIONS
    // ============================================================

    pub async fn create<T: ODModel>(&self, model: &mut T) -> Result<(), ForgeError> {
        model.set_id(ObjectId::new());
        model.before_create()?;
        let _ = self.coll::<T>(T::coll_name()).insert_one(&*model).await.map_err(|e| {
            let msg = format!("Failed to create document");
            log::error(&msg, Some(json!({ "mod":"odm::create","error": e.to_string() })));
            ForgeError::internal().message(msg).caused_by(e)
        })?;
        model.after_create()?;
        Ok(())
    }

    pub async fn update<T: ODModel>(&self, model: &mut T) -> Result<(), ForgeError> {
        model.before_update()?;
        let mut data = to_document(&*model).map_err(|e| {
            let msg = format!("Failed to serialize document");
            log::error(&msg, Some(json!({ "mod":"odm::update","error": e.to_string() })));
            ForgeError::internal().message(msg).caused_by(e)
        })?;
        data.remove("_id");
        let id = model.get_id().ok_or_else(|| {
            let msg = "Document id is None";
            log::error(msg, Some(json!({ "mod":"odm::update","error": "Document id is None" })));
            ForgeError::not_found().message(msg)
        })?;

        let res = self
            .coll::<T>(T::coll_name())
            .update_one(doc! {"_id": id}, data)
            .await
            .map_err(|e| {
                let msg = format!("Failed to update document [{}]", id);
                log::error(&msg, Some(json!({ "mod":"odm::update","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;

        if res.matched_count == 0 {
            let msg = format!("Document [{}] not found", id);
            log::warning(
                &msg,
                Some(json!({ "mod":"odm::update","error": "res.matched_count == 0" })),
            );
            return Err(ForgeError::not_found().message(msg));
        }
        if res.modified_count == 0 {
            return Ok(());
        }

        model.after_update()?;
        Ok(())
    }

    pub async fn save<T: ODModel>(&self, model: &mut T) -> Result<(), ForgeError> {
        match model.get_id() {
            Some(_) => self.update(model).await,
            None => self.create(model).await,
        }
    }

    pub async fn replace<T: ODModel>(&self, model: &mut T) -> Result<(), ForgeError> {
        model.before_replace()?;
        let id = model.get_id().ok_or_else(|| {
            let msg = "Document id is None";
            log::error(
                msg,
                Some(json!({ "mod":"odm::replace","error": "Document id is None" })),
            );
            ForgeError::not_found().message(msg)
        })?;

        let res = self
            .coll::<T>(T::coll_name())
            .replace_one(doc! {"_id": id}, &*model)
            .await
            .map_err(|e| {
                let msg = format!("Failed to replace document");
                log::error(&msg, Some(json!({ "mod":"odm::replace","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;

        if res.matched_count == 0 {
            let msg = format!("Document [{}] not found", id);
            log::warning(
                &msg,
                Some(json!({ "mod":"odm::replace","error": "res.matched_count == 0" })),
            );
            return Err(ForgeError::not_found().message(msg));
        }

        model.after_replace()?;
        Ok(())
    }

    pub async fn delete<T: ODModel>(&self, model: &mut T) -> Result<(), ForgeError> {
        model.before_delete()?;
        let id = model.get_id().ok_or_else(|| {
            let msg = "Document id is None";
            log::error(msg, Some(json!({ "mod":"odm::delete","error": "Document id is None" })));
            ForgeError::not_found().message(msg)
        })?;

        let res = self
            .coll::<T>(T::coll_name())
            .delete_one(doc! {"_id": id})
            .await
            .map_err(|e| {
                let msg = format!("Failed to delete document");
                log::error(&msg, Some(json!({ "mod":"odm::delete","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;

        if res.deleted_count == 0 {
            let msg = format!("Document [{}] not found", id);
            log::warning(
                &msg,
                Some(json!({ "mod":"odm::delete","error": "res.deleted_count == 0" })),
            );
            return Err(ForgeError::not_found().message(msg));
        }

        model.after_delete()?;
        Ok(())
    }

    // ============================================================
    // CRUD MANY OPERATIONS
    // ============================================================

    pub async fn create_many<T: ODModel>(&self, docs: &mut Vec<T>) -> Result<(), ForgeError> {
        let res = self
            .coll::<T>(T::coll_name())
            .insert_many(docs.iter())
            .await
            .map_err(|e| {
                let msg = "Failed to create many documents".to_string();

                // Intenta extraer detalle estructurado del error, si el driver lo expone
                let error_detail = json!({
                    "mod": "odm::create_many",
                    "error": e.to_string(),
                    "kind": format!("{:?}", e.kind), // Debug del ErrorKind completo, suele traer más contexto
                });

                log::error(&msg, Some(error_detail.clone()));

                ForgeError::internal().message(msg).caused_by(e).with_data(error_detail)
            })?;

        let mut errs: Vec<String> = Vec::new();
        for (k, v) in res.inserted_ids {
            match v.as_object_id() {
                Some(id) => docs[k].set_id(id),
                None => {
                    let msg = format!("the document number [{}] has no id", k);
                    log::error(
                        &msg,
                        Some(json!({ "mod":"odm::create_many","error": "v.as_object_id() == None" })),
                    );
                    errs.push(msg);
                }
            };
        }

        if errs.len() > 0 {
            return Err(ForgeError::internal()
                .message("failed to get inserted ids")
                .with_data(errs));
        }
        Ok(())
    }

    pub async fn update_many<T: ODModel>(
        &self,
        filter: Document,
        update: Document,
    ) -> Result<UpdateResult, ForgeError> {
        let res = self
            .coll::<T>(T::coll_name())
            .update_many(filter, update)
            .await
            .map_err(|e| {
                let msg = format!("Failed to update many documents");
                log::error(&msg, Some(json!({ "mod":"odm::update_many","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        // if res.matched_count == 0 {
        //     let msg = format!("No documents matched the filter");
        //     log::warning(
        //         &msg,
        //         Some(json!({ "mod":"odm::update_many","error": "res.matched_count == 0", "filter": filter })),
        //     );
        //     return Err(ForgeError::not_found().message(msg));
        // }
        Ok(res)
    }

    // ============================================================
    // CRUD OPERATIONS · GETTING PREVIOUS
    // ============================================================

    pub async fn update_getting_previous<T: ODModel>(&self, model: &mut T) -> Result<T, ForgeError> {
        let id = model.get_id().ok_or_else(|| {
            let msg = "Document id is None";
            log::error(
                msg,
                Some(json!({ "mod":"odm::update_getting_previous","error": "Document id is None" })),
            );
            ForgeError::not_found().message(msg)
        })?;
        let mut data = to_document(&*model).map_err(|e| {
            let msg = format!("Failed to serialize document");
            log::error(
                &msg,
                Some(json!({ "mod":"odm::update_getting_previous","error": e.to_string() })),
            );
            ForgeError::internal().message(msg).caused_by(e)
        })?;
        data.remove("_id");
        let res = self
            .coll::<T>(T::coll_name())
            .find_one_and_update(doc! {"_id": id}, data)
            .await
            .map_err(|e| {
                let msg = format!("Failed to update document: [{}]", id);
                log::error(
                    &msg,
                    Some(json!({ "mod":"odm::update_getting_previous","error": e.to_string() })),
                );
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        match res {
            Some(data) => Ok(data),
            None => {
                let msg = format!("Document [{}] not found", id);
                log::error(&msg, Some(json!({ "mod":"odm::update_getting_previous","error": msg })));
                Err(ForgeError::not_found().message(msg))
            }
        }
    }

    pub async fn replace_getting_previous<T: ODModel>(&self, model: &mut T) -> Result<T, ForgeError> {
        let id = model.get_id().ok_or_else(|| {
            let msg = "Document id is None";
            log::error(
                msg,
                Some(json!({ "mod":"odm::replace_getting_previous","error": "Document id is None" })),
            );
            ForgeError::not_found().message(msg)
        })?;
        let res = self
            .coll::<T>(T::coll_name())
            .find_one_and_replace(doc! {"_id": id}, &*model)
            .await
            .map_err(|e| {
                let msg = format!("Failed to replace document: [{}]", id);
                log::error(
                    &msg,
                    Some(json!({ "mod":"odm::replace_getting_previous","error": e.to_string() })),
                );
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        match res {
            Some(data) => Ok(data),
            None => {
                let msg = format!("Document [{}] not found", id);
                log::error(
                    &msg,
                    Some(json!({ "mod":"odm::replace_getting_previous","error": msg })),
                );
                Err(ForgeError::not_found().message(msg))
            }
        }
    }

    pub async fn delete_getting_previous<T: ODModel>(&self, id: ObjectId) -> Result<T, ForgeError> {
        let res = self
            .coll::<T>(T::coll_name())
            .find_one_and_delete(doc! {"_id": id})
            .await
            .map_err(|e| {
                let msg = format!("Failed to delete document");
                log::error(
                    &msg,
                    Some(json!({ "mod":"odm::delete_getting_previous","error": e.to_string() })),
                );
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        match res {
            Some(data) => Ok(data),
            None => {
                let msg = format!("Document [{}] not found", id);
                log::warning(
                    &msg,
                    Some(json!({ "mod":"odm::delete_getting_previous","error": "res.deleted_count == 0" })),
                );
                Err(ForgeError::not_found().message(msg))
            }
        }
    }

    // ============================================================
    // FIND OPERATIONS
    // ============================================================

    pub async fn get_by_id<T: ODModel>(&self, id: ObjectId) -> Result<T, ForgeError> {
        let res = self
            .coll::<T>(T::coll_name())
            .find_one(doc! {"_id": id})
            .await
            .map_err(|e| {
                let msg = format!("Failed to find document [{}]", id);
                log::error(&msg, Some(json!({ "mod":"odm::get_by_id","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        match res {
            Some(data) => Ok(data),
            None => {
                let msg = format!("Document [{}] not found", id);
                log::error(&msg, Some(json!({ "mod":"odm::get_by_id","error": msg })));
                Err(ForgeError::not_found().message(msg))
            }
        }
    }

    pub async fn get_by_hex_id<T: ODModel>(&self, hex_id: &str) -> Result<T, ForgeError> {
        let id = ODMongo::hex_to_object_id(hex_id)?;
        self.get_by_id(id).await
    }

    pub async fn get_all<T: ODModel>(&self) -> Result<Vec<T>, ForgeError> {
        let mut cursor = self.coll::<T>(T::coll_name()).find(doc! {}).await.map_err(|e| {
            let msg = format!("Failed to find documents");
            log::error(&msg, Some(json!({ "mod":"odm::get_all","error": e.to_string() })));
            ForgeError::internal().message(msg).caused_by(e)
        })?;
        let mut docs: Vec<T> = Vec::new();
        while let Some(doc) = cursor.try_next().await.map_err(|e| {
            let msg = format!("Failed to iterate over documents");
            log::error(&msg, Some(json!({ "mod":"odm::get_all","error": e.to_string() })));
            ForgeError::internal().message(msg).caused_by(e)
        })? {
            docs.push(doc);
        }
        Ok(docs)
    }

    pub fn find<T, U>(&self, filter: Document) -> ODMFind<T, U>
    where
        T: ODModel,
        U: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        ODMFind::new(self.clone(), filter)
    }

    // ============================================================
    // HELPER FUNCTIONS
    // ============================================================

    pub fn hex_to_object_id(hex: &str) -> Result<ObjectId, ForgeError> {
        ObjectId::from_str(hex).map_err(|e| {
            let msg = format!("Invalid id [{}]", hex);
            log::error(
                &msg,
                Some(json!({ "mod":"odm::hex_to_object_id","error": e.to_string() })),
            );
            ForgeError::internal().message(msg).caused_by(e)
        })
    }
}

pub fn odm() -> Result<&'static ODMongo, ForgeError> {
    match ODMONGO.get() {
        Some(odm) => Ok(odm),
        None => Err(ForgeError::internal().message("DB not initialized")),
    }
}
