use cached::proc_macro::cached;
use cached::SizedCache;
use mongodb::sync::Client;
use mongodb::bson::doc;
use serde::Deserialize;

// Define a struct to hold your config data
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub partner_id: String,
    pub service_uri: String,
    pub service_name: String,
}

// Function to get the config from MongoDB
fn get_config_from_db(
    partner_id: String,
    service_name: String,
) -> mongodb::error::Result<Option<Config>> {
    let client = Client::with_uri_str("mongodb://localhost:27017");
    let database = client?.database("partner");
    let collection = database.collection::<Config>("config");

    // Query the database for the config document
    // Assuming there's only one config document
    let filter = doc! {
        "partner_id": partner_id,
        "service_name": service_name
    };

    collection.find_one(filter, None)
}

// Cached function to get the config
#[cached(
    ty = "SizedCache<String, Option<Config>>",
    create = "{ SizedCache::with_size(100) }",
    convert = r#"{ format!("{}-#-{}", partner_id, service_name) }"#
)]
pub fn get_cached_config(partner_id: String, service_name: String) -> Option<Config> {
    match get_config_from_db(partner_id, service_name) {
        Ok(conf) => conf,
        Err(error) => {
            println!("Error in Mongo: {}", error.to_string());
            None
        }
    }
}
