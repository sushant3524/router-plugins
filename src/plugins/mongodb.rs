use cached::proc_macro::cached;
use cached::SizedCache;
use mongodb::error::Error;
use mongodb::{bson::doc, options::ClientOptions, Client};
use serde::Deserialize;

// Define a struct to hold your config data
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub partner_id: String,
    pub partner_graph_url: String,
}

// Initialize MongoDB connection
async fn init_mongo() -> mongodb::error::Result<Client> {
    let mongo_uri = "mongodb://localhost:27017/";
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = Client::with_options(client_options)?;
    Ok(client)
}

// Function to get the config from MongoDB
async fn get_config_from_db(partner_id: String) -> mongodb::error::Result<Config> {
    let client = init_mongo().await?;
    let database = client.database("partner");
    let collection = database.collection::<Config>("config");

    // Query the database for the config document
    // Assuming there's only one config document
    let filter = doc! {
        "partner_id": partner_id
    };

    let config = collection.find_one(filter, None).await?.unwrap();
    Ok(config)
}

// Cached function to get the config
#[cached(
    ty = "SizedCache<String, Result<Config, Error>>",
    create = "{ SizedCache::with_size(100) }",
    convert = r#"{ format!("{}", partner_id) }"#
)]
pub async fn get_cached_config(partner_id: String) -> Result<Config, Error> {
    get_config_from_db(partner_id).await
}
