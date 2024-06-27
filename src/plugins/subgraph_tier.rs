use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::ControlFlow;

use apollo_router::graphql;
use apollo_router::layers::ServiceBuilderExt;
use apollo_router::plugin::Plugin;
use apollo_router::plugin::PluginInit;
use apollo_router::register_plugin;
use apollo_router::services::execution;
use apollo_router::services::router;
use apollo_router::services::subgraph;
use apollo_router::services::supergraph;
use apollo_router::Context;
use cached::Cached;
use http::StatusCode;
use http::Uri;
use schemars::JsonSchema;
use serde::Deserialize;
use std::str;
use tower::BoxError;
use tower::ServiceBuilder;
use tower::ServiceExt;

use crate::plugins::mongodb::get_cached_config;
use crate::plugins::mongodb::CONFIG_CACHE;

#[derive(Debug)]
struct SubgraphTiering {
    default_service_uris: HashMap<String, Uri>,
    default_partner_id: String,
    cache_header_key: String,
}
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
struct ServiceDefaults {
    name: String,
    default_uri: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct Conf {
    // Put your plugin configuration here. It will automatically be deserialized from JSON.
    // Always put some sort of config here, even if it is just a bool to say that the plugin is enabled,
    // otherwise the yaml to enable the plugin will be confusing.
    message: String,
    services: Vec<ServiceDefaults>,
    default_partner_id: String,
    cache_header_key: String,
}

#[async_trait::async_trait]
impl Plugin for SubgraphTiering {
    type Config = Conf;

    async fn new(init: PluginInit<Self::Config>) -> Result<Self, BoxError> {
        // print message on plugin startup
        tracing::info!("{}", init.config.message);

        // create a map of service names (for different subgraphs) and default urls
        let mut hash_map: HashMap<String, Uri> = HashMap::new();
        for s in init.config.services.iter().cloned() {
            hash_map.insert(s.name, s.default_uri.parse::<Uri>().unwrap());
        }

        Ok(SubgraphTiering {
            default_service_uris: hash_map,
            default_partner_id: init.config.default_partner_id,
            cache_header_key: init.config.cache_header_key,
        })
    }

    // Delete this function if you are not customizing it.
    fn router_service(&self, service: router::BoxService) -> router::BoxService {
        let cache_header_key: String = self.cache_header_key.clone();
        ServiceBuilder::new()
            .checkpoint(move |request: router::Request| {
                Ok(cache_control(request, cache_header_key.borrow()))
            })
            .service(service)
            .boxed()
    }

    // Delete this function if you are not customizing it.
    fn supergraph_service(&self, service: supergraph::BoxService) -> supergraph::BoxService {
        // Always use service builder to compose your plugins.
        // It provides off the shelf building blocks for your plugin.
        //
        // ServiceBuilder::new()
        //             .service(service)
        //             .boxed()

        // Returning the original service means that we didn't add any extra functionality for at this point in the lifecycle.
        service
    }

    // Delete this function if you are not customizing it.
    fn execution_service(&self, service: execution::BoxService) -> execution::BoxService {
        service
    }

    // Note that this function panics if no default value for a subgraph is provided
    // This is not a good behavior to start out ... but I have added this so that
    // Config files are for this are not forgotten or misspelt
    // You can get default value from `enum join__Graph` in your supergraph
    fn subgraph_service(&self, _name: &str, service: subgraph::BoxService) -> subgraph::BoxService {
        let service_name = _name.to_string();
        let default_uri = self.default_service_uris.get(_name);
        let default_partner_id = self.default_partner_id.clone();
        let uri: Uri = match default_uri {
            Some(value) => value.clone(),
            None => panic!("ERROR: default uri for {} not provided for", service_name), // TODO: add proper logging
        };

        ServiceBuilder::new()
            .map_request(move |mut request: subgraph::Request| {
                let partner_id_header = request.subgraph_request.headers().get("PARTNER-ID");
                let partner_id = match partner_id_header {
                    Some(id) => {
                        match str::from_utf8(id.as_bytes()) {
                            Ok(value) => value,
                            Err(err) => {
                                println!("WARN: {}", err); // TODO: add proper logging
                                &default_partner_id
                            }
                        }
                    }
                    None => &default_partner_id,
                };
                let partner_id = partner_id.to_string();

                let ru = request.subgraph_request.uri_mut();
                let config = get_cached_config(partner_id, service_name.clone());

                match config {
                    Some(conf) => {
                        *ru = match conf.service_uri.parse::<Uri>() {
                            Ok(uri_from_config) => uri_from_config,
                            Err(_) => uri.clone(),
                        }
                    }
                    None => *ru = uri.clone(),
                }

                return request;
            })
            .service(service)
            .boxed()
    }
}

// This macro allows us to use it in our plugin registry!
// register_plugin takes a group name, and a plugin name.
register_plugin!("starstruck", "subgraph_tier", SubgraphTiering);

fn cache_control(
    request: router::Request,
    cache_header_key: &String,
) -> ControlFlow<router::Response, router::Request> {
    // We are going to do a lot of similar checking so let's define a local function
    // to help reduce repetition
    fn cancel_message(
        context: Context,
        error_message: String,
        status: StatusCode,
    ) -> ControlFlow<router::Response, router::Request> {
        let response = router::Response::builder()
            .error(
                graphql::Error::builder()
                    .message(error_message)
                    .extension_code("CACHE") // TODO: add key to constants
                    .build(),
            )
            .status_code(status)
            .context(context)
            .build()
            .unwrap();
        ControlFlow::Break(response)
    }

    let clear_cache_header = request.router_request.headers().get(cache_header_key);
    let clear_cache = match clear_cache_header {
        Some(_) => true,
        None => false,
    };

    if !clear_cache {
        return ControlFlow::Continue(request);
    } else {
        {
            let mut cache = CONFIG_CACHE.lock().unwrap();
            cache.cache_clear();
        }

        cancel_message(
            request.context,
            "cleared cache".to_string(),
            StatusCode::ACCEPTED,
        )
    }
}

#[cfg(test)]
mod tests {
    use apollo_router::graphql;
    use apollo_router::services::supergraph;
    use apollo_router::TestHarness;
    use tower::BoxError;
    use tower::ServiceExt;

    #[tokio::test]
    async fn basic_test() -> Result<(), BoxError> {
        let test_harness = TestHarness::builder()
            .configuration_json(serde_json::json!({
                "plugins": {
                    "starstruck.subgraph_tier": {
                        "message" : "Starting my plugin"
                    }
                }
            }))
            .unwrap()
            .build_router()
            .await
            .unwrap();
        let request = supergraph::Request::canned_builder().build().unwrap();
        let mut streamed_response = test_harness.oneshot(request.try_into()?).await?;

        let first_response: graphql::Response = serde_json::from_slice(
            streamed_response
                .next_response()
                .await
                .expect("couldn't get primary response")?
                .to_vec()
                .as_slice(),
        )
        .unwrap();

        assert!(first_response.data.is_some());

        println!("first response: {:?}", first_response);
        let next = streamed_response.next_response().await;
        println!("next response: {:?}", next);

        // You could keep calling .next_response() until it yields None if you're expexting more parts.
        assert!(next.is_none());
        Ok(())
    }
}
