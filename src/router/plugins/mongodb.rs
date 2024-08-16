use cached::Cached;
use cached::SizedCache;
use futures::TryFutureExt;
use mongodb::bson::doc;
use mongodb::options::ClientOptions;
use mongodb::sync::Client;
use serde::{Deserialize, Serialize};
use reqwest::Error;

pub static CONFIG_CACHE: once_cell::sync::Lazy<
    std::sync::Mutex<SizedCache<String, Option<Config>>>,
> = once_cell::sync::Lazy::new(|| {
    let cache_size = match std::env::var("DEFAULT_URI_CACHE_SIZE")
        .expect("Missing URI_CACHE_SIZE environment variable")
        .parse::<usize>()
    {
        Ok(value) => value,
        Err(err) => panic!("Could not create cache because {err}"),
    };
    std::sync::Mutex::new(SizedCache::with_size(cache_size))
});

static MONGO_CLIENT: once_cell::sync::Lazy<mongodb::sync::Client> =
    once_cell::sync::Lazy::new(|| {
        let mongo_url =
            std::env::var("MONGODB_URI").expect("Missing MONGO_URL environment variable");
        let client_options = ClientOptions::parse(&mongo_url)
            .expect("Client options provided could not be parsed properly");
        match Client::with_options(client_options) {
            Ok(client) => client,
            Err(err) => panic!("Could not create a Mongo Client as {err}"),
        }
    });

// Define a struct to hold your config data
#[derive(Deserialize, Clone)]
pub struct Config {
    pub partner_id: String,
    pub service_uri: String,
    pub service_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse {
    r#type: String,
    result: ApiResult,
    #[serde(default)]
    stackTrace: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ApiResult {
    Success { url: String },
    Failure(String),
}

// Function to get the config from MongoDB
fn get_config_from_tier_configuration(
    partnerId: String,
    service: String,
) -> mongodb::error::Result<Option<Config>> {
    tracing::info!("[TEST-SUSH] service {:?}", service);
    let api_url = format!(
        "http://qa6-restricted-tier2.sprinklr.com/restricted/v1/care/feature/get-url-for-service/{}/{}",
        partnerId, service
    );

    let client = reqwest::Client::new();
    match client
        .post(&api_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send()
        .and_then(|res| res.json::<ApiResponse>()) // Deserialize the response
    {
        Ok(response) => match response.r#type.as_str() {
            "SUCCESS" => {
                tracing::info!("[TEST-SUSH] success service {:?}", service);
                if let ApiResult::Success { url } = response.result {
                    let config = Config {
                        partner_id,
                        service_uri: url,
                        service_name: service,
                    };
                    Ok(Some(config))
                } else {
                    Ok(None)
                }
            }
            "FAILED" => {
                tracing::info!("[TEST-SUSH] failed service {:?}", service);
                println!("Failed to get URL: {:?}", response.result);
                Ok(None)
            }
            _ => {
                tracing::info!("[TEST-SUSH] _ service {:?}", service);
                println!("Unexpected response: {:?}", response);
                Ok(None)
            }
        },
        Err(e) => {
            tracing::info!("[TEST-SUSH] error service {:?}", e);
            // Handle any kind of error and return `Ok(None)`
            println!("Error occurred during API call: {:?}", e);
            Ok(None)
        }
    }
}

// Note that this function does not cache if the config is not found
// i.e. if the config is not found, the next function call will not return 'None' directly
// but will query the database again. This might be inconvenient for performance
// but should encourage storing tier-config for all partners to the database
pub fn get_cached_config(partner_id: String, service_name: String) -> Option<Config> {
    let key = format!("{0}-#-{1}", partner_id, service_name);

    {
        let mut cache = CONFIG_CACHE.lock().unwrap();
        if let Some(config) = cache.cache_get(&key) {
            return config.to_owned();
        }
    }

    match get_config_from_tier_configuration(partner_id, service_name) {
        Ok(config) => {
            {
                let mut cache = CONFIG_CACHE.lock().unwrap();
                cache.cache_set(key, config.to_owned());
            }

            config
        }
        Err(error) => {
            println!("Error in Mongo: {}", error.to_string());
            None
        }
    }
}
