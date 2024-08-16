use cached::Cached;
use cached::SizedCache;
use mongodb::options::ClientOptions;
use reqwest::blocking::Client;
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

// Define a struct to hold your config data
#[derive(Deserialize, Clone)]
pub struct Config {
    pub partner_id: String,
    pub service_uri: String,
    pub service_name: String,
}

#[derive(Deserialize, Debug)]
struct ApiResponse {
    r#type: String,
    result: Option<ResultField>,
}

#[derive(Deserialize, Debug)]
struct ResultField {
    url: Option<String>,
}

// Function to get the config from MongoDB
fn get_config_from_tier_configuration(
    partnerId: String,
    service: String,
) -> Option<Config> {
    tracing::info!("[TEST-SUSH] service {:?}", service);
    let api_url = format!(
        "http://qa6-restricted-tier2.sprinklr.com/restricted/v1/care/feature/get-url-for-service/{}/{}",
        partnerId, service
    );

    let response = Client::new()
        .post(&api_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send();

    // Handle the response
    match response {
        Ok(resp) => {
            tracing::info!("[TEST-SUSH] service ok {:?}", service);
            if resp.status().is_success() {
                let api_response: Result<ApiResponse, Error> = resp.json();
                tracing::info!("[TEST-SUSH] service success {:?}", service);
                match api_response {
                    Ok(data) => {
                        tracing::info!("[TEST-SUSH] service response ok {:?}", service);
                        // Check if the response type is "SUCCESS" and return the Config if URL exists
                        if data.r#type == "SUCCESS" {
                            tracing::info!("[TEST-SUSH] service response ok sucess {:?}", service);
                            if let Some(result_field) = data.result {
                                tracing::info!("[TEST-SUSH] service response ok success result_field {:?}", result_field);
                                if let Some(service_uri) = result_field.url {
                                    tracing::info!("[TEST-SUSH] service response ok success service_uri {:?}", service_uri);
                                    return Some(Config {
                                        partner_id: partnerId,
                                        service_uri,
                                        service_name: service,
                                    });
                                }
                            }
                        }
                    }
                    Err(error) => {
                        // Handle JSON deserialization error
                        tracing::info!("[TEST-SUSH] Handle JSON deserialization error {:?}", error);
                        return None;
                    }
                }
            }
        }
        Err(err) => {
            // Handle HTTP request error
            tracing::info!("[TEST-SUSH] Handle HTTP request error {:?}", err);
            return None;
        }
    }
    tracing::info!("[TEST-SUSH] nothing matters");
    // Return None if the API call fails or response type is "FAILED"
    None
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
        Some(config) => {
            {
                let mut cache = CONFIG_CACHE.lock().unwrap();
                cache.cache_set(key, Option::from(config.to_owned()));
            }

            Option::from(config)
        }
        None => {
            println!("None when trying to retrieve tier");
            None
        }
    }
}
