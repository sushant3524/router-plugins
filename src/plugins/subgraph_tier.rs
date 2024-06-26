use std::collections::HashMap;

use apollo_router::plugin::Plugin;
use apollo_router::plugin::PluginInit;
use apollo_router::register_plugin;
use apollo_router::services::execution;
use apollo_router::services::router;
use apollo_router::services::subgraph;
use apollo_router::services::supergraph;
use std::str;
use http::Uri;
use schemars::JsonSchema;
use serde::Deserialize;
use tower::BoxError;
use tower::ServiceBuilder;
use tower::ServiceExt;

use crate::plugins::mongodb::get_cached_config;

#[derive(Debug)]
struct SubgraphTiering {
    configuration: Conf,
    default_service_uris: HashMap<String, Uri>,
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
    service: Vec<ServiceDefaults>,
}

#[async_trait::async_trait]
impl Plugin for SubgraphTiering {
    type Config = Conf;

    async fn new(init: PluginInit<Self::Config>) -> Result<Self, BoxError> {
        tracing::info!("{}", init.config.message);
        let mut hash_map: HashMap<String, Uri> = HashMap::new();
        for s in init.config.service.iter().cloned() {
            hash_map.insert(s.name, s.default_uri.parse::<Uri>().unwrap());
        }
        for s in init.config.service.iter().cloned() {
            tracing::info!("{}", s.default_uri);
        }

        Ok(SubgraphTiering {
            configuration: init.config,
            default_service_uris: hash_map,
        })
    }

    // Delete this function if you are not customizing it.
    fn router_service(&self, service: router::BoxService) -> router::BoxService {
        // Always use service builder to compose your plugins.
        // It provides off the shelf building blocks for your plugin.
        //
        // ServiceBuilder::new()
        //             .service(service)
        //             .boxed()

        // Returning the original service means that we didn't add any extra functionality at this point in the lifecycle.
        service
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

    // Delete this function if you are not customizing it.
    fn subgraph_service(&self, _name: &str, service: subgraph::BoxService) -> subgraph::BoxService {
        let service_name = _name.to_string();
        let default_uri = self.default_service_uris.get(_name);
        let uri: Uri;

        match default_uri {
            Some(value) => uri = value.clone(),
            None => panic!("default uri for {} not provided for", service_name)
        }

        ServiceBuilder::new()
            .map_request(move |mut request: subgraph::Request| {
                let partner_id_header = request.subgraph_request.headers_mut().get("PARTNER-ID");
                let partner_id = match partner_id_header {
                    Some(id) => {
                        match str::from_utf8(id.as_bytes()) {
                            Ok(value) => value,
                            Err(err) => {
                                println!("WARN: {}", err);
                                "1"
                            }
                        }
                    },
                    None => "1" 
                };
                let partner_id = partner_id.to_string();

                let ru = request.subgraph_request.uri_mut();
                let config =
                    get_cached_config(partner_id, service_name.clone());

                match config {
                    Some(conf) => *ru = conf.service_uri.parse::<Uri>().unwrap(),
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
