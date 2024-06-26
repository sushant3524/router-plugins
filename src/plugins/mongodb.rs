use cached::Cached;
use cached::SizedCache;
use mongodb::bson::doc;
use mongodb::sync::Client;
use serde::Deserialize;

pub static CONFIG_CACHE: ::cached::once_cell::sync::Lazy<
    std::sync::Mutex<SizedCache<String, Config>>,
> = ::cached::once_cell::sync::Lazy::new(|| std::sync::Mutex::new(SizedCache::with_size(100)));

// Define a struct to hold your config data
#[derive(Deserialize, Clone)]
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

// Note that this function does not cache if the config is not found
// i.e. if the config is not found, the next function call will not return 'None' directly
// but will query the database again. This might be inconvenient for performance
// but should encourage storing tier-config for all partners to the database
pub fn get_cached_config(partner_id: String, service_name: String) -> Option<Config> {
    let key = format!("{0}-#-{1}", partner_id, service_name);
    let mut cache = CONFIG_CACHE.lock().unwrap();
    if let Some(result) = cache.cache_get(&key) {
        return Some(result.to_owned());
    }

    match get_config_from_db(partner_id, service_name) {
        Ok(result) => match result {
            Some(conf) => {
                cache.cache_set(key, conf.clone());
                Some(conf)
            }
            None => None,
        },
        Err(error) => {
            println!("Error in Mongo: {}", error.to_string());
            None
        }
    }
}
