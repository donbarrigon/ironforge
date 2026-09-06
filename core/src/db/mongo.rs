use mongodb::{Client, Collection, Database};
use std::sync::OnceLock;

use crate::{ForgeError, env, log};

static CLIENT: OnceLock<Client> = OnceLock::new();
// static DB: OnceLock<Database> = OnceLock::new();

pub async fn init_mongo() -> Result<(), ForgeError> {
    let uri = env().db.string_connection.clone();
    // let db_name = env().db.name.clone();

    let client = Client::with_uri_str(&uri).await.map_err(|e| {
        let msg = format!("Failed to connect/ping MongoDB: {}", e);
        log::error(&msg, None);
        ForgeError::internal().message(msg).caused_by(e)
    })?;

    let _ = client.list_database_names().await.map_err(|e| {
        let msg = format!("Failed to connect/ping MongoDB: {}", e);
        log::error(&msg, None);

        ForgeError::internal().message(msg).caused_by(e)
    })?;
    let _ = CLIENT.set(client);

    // let db = match CLIENT.get() {
    //     Some(client) => client.database(&db_name),
    //     None => {
    //         let msg = "MongoDB client not initialized";
    //         log::error(msg, None);
    //         return Err(ForgeError::internal().message(msg));
    //     }
    // };
    // let _ = DB.set(db);

    log::info("MongoDB initialized successfully", None);
    Ok(())
}

pub fn db() -> Result<Database, ForgeError> {
    match CLIENT.get() {
        Some(client) => Ok(client.database(&env().db.name)),
        None => Err(ForgeError::internal().message("Client not initialized")),
    }
}

pub fn coll<T: Send + Sync>(name: &str) -> Result<Collection<T>, ForgeError> {
    match db() {
        Ok(db) => Ok(db.collection::<T>(name)),
        Err(e) => Err(e),
    }
}
