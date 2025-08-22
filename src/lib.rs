#![deny(clippy::all)]

use napi_derive::napi;

mod store;

#[napi]
pub fn store_init(db_path: String) -> Result<String, napi::Error> {
    match store::init(&db_path) {
        Ok(_) => Ok("store initialized successfully".to_string()),
        Err(e) => Err(napi::Error::from_reason(format!("Failed to initialize store: {}", e))),
    }
}
