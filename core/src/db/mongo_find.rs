use futures::TryStreamExt;
use mongodb::{
    Cursor,
    bson::Document,
    options::{FindOneOptions, FindOptions},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

use crate::{
    ForgeError,
    db::{mongo_model::ODModel, mongo_odm::ODMongo},
    log,
};

pub struct ODMFind<T: ODModel, U: Serialize + DeserializeOwned + Send + Sync + 'static = T> {
    filter: Document,
    skip: Option<u64>,
    limit: Option<i64>,
    sort: Option<Document>,
    projection: Option<Document>,
    odm: ODMongo,
    _marker_t: std::marker::PhantomData<T>,
    _marker_u: std::marker::PhantomData<U>,
}

impl<T: ODModel, U: Serialize + DeserializeOwned + Send + Sync + 'static> ODMFind<T, U> {
    pub fn new(odm: ODMongo, filter: Document) -> Self {
        Self {
            filter: filter,
            skip: None,
            limit: None,
            sort: None,
            projection: None,
            odm: odm,
            _marker_t: std::marker::PhantomData,
            _marker_u: std::marker::PhantomData,
        }
    }

    pub fn filter(mut self, filter: Document) -> Self {
        self.filter = filter;
        self
    }

    pub fn skip(mut self, skip: u64) -> Self {
        self.skip = Some(skip);
        self
    }

    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn sort(mut self, sort: Document) -> Self {
        self.sort = Some(sort);
        self
    }

    pub fn paginate(mut self, page: u64, limit: u64) -> Self {
        self.skip = Some((page - 1) * limit);
        self.limit = Some(limit as i64);
        self
    }

    pub fn projection(mut self, projection: Document) -> Self {
        self.projection = Some(projection);
        self
    }

    pub async fn first(self) -> Result<U, ForgeError> {
        let opions = FindOneOptions::builder()
            .sort(self.sort.clone())
            .projection(self.projection.clone())
            .build();
        let res = self
            .odm
            .coll::<U>(T::coll_name())
            .find_one(self.filter)
            .with_options(opions)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find documents");
                log::error(&msg, Some(json!({ "mod":"odm::find::first","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        match res {
            Some(data) => Ok(data),
            None => {
                let msg = format!("Document not found");
                log::error(&msg, Some(json!({ "mod":"odm::find::first","error": msg })));
                Err(ForgeError::not_found().message(msg))
            }
        }
    }

    pub async fn get(self) -> Result<Vec<U>, ForgeError> {
        let options = self.build_options();

        let mut cursor = self
            .odm
            .coll::<U>(T::coll_name())
            .find(self.filter)
            .with_options(options)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find documents");
                log::error(&msg, Some(json!({ "mod":"odm::find::get","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;

        let mut docs: Vec<U> = Vec::new();
        while let Some(doc) = cursor.try_next().await.map_err(|e| {
            let msg = format!("Failed to iterate over documents");
            log::error(&msg, Some(json!({ "mod":"odm::find::get","error": e.to_string() })));
            ForgeError::internal().message(msg).caused_by(e)
        })? {
            docs.push(doc);
        }
        Ok(docs)
    }

    pub async fn exec(self) -> Result<Cursor<U>, ForgeError> {
        let options = self.build_options();

        let cursor = self
            .odm
            .coll::<U>(T::coll_name())
            .find(self.filter)
            .with_options(options)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find documents");
                log::error(&msg, Some(json!({ "mod":"odm::find::exec","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        Ok(cursor)
    }

    pub async fn count(&self) -> Result<u64, ForgeError> {
        let count = self
            .odm
            .coll::<T>(T::coll_name())
            .count_documents(self.filter.clone())
            .await
            .map_err(|e| {
                let msg = format!("Failed to count documents");
                log::error(&msg, Some(json!({ "mod":"odm::find::count","error": e.to_string() })));
                ForgeError::internal().message(msg).caused_by(e)
            })?;
        Ok(count)
    }

    fn build_options(&self) -> FindOptions {
        let mut options = mongodb::options::FindOptions::default();
        options.skip = self.skip;
        options.limit = self.limit;
        options.sort = self.sort.clone();
        options.projection = self.projection.clone();
        options
    }
}
