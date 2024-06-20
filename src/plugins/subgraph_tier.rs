use apollo_router::plugin::Plugin;
use apollo_router::plugin::PluginInit;
use apollo_router::register_plugin;
use apollo_router::services::execution;
use apollo_router::services::router;
use apollo_router::services::subgraph;
use apollo_router::services::supergraph;
use futures::executor;
use http::Uri;
use schemars::JsonSchema;
use serde::Deserialize;
use tower::BoxError;
use tower::ServiceBuilder;
use tower::ServiceExt;

use crate::plugins::mongodb::get_cached_config;

#[derive(Debug)]
struct SubgraphTiering {
    #[allow(dead_code)]
    configuration: Conf,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct Conf {
    // Put your plugin configuration here. It will automatically be deserialized from JSON.
    // Always put some sort of config here, even if it is just a bool to say that the plugin is enabled,
    // otherwise the yaml to enable the plugin will be confusing.
    message: String,
}
// This is a bare bones plugin that can be duplicated when creating your own.
#[async_trait::async_trait]
impl Plugin for SubgraphTiering {
    type Config = Conf;

    async fn new(init: PluginInit<Self::Config>) -> Result<Self, BoxError> {
        tracing::info!("{}", init.config.message);
        Ok(SubgraphTiering {
            configuration: init.config,
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
        ServiceBuilder::new()
            .map_request(|mut request: subgraph::Request| {
                println!("{}", request.subgraph_request.uri());

                // logic for changing subgraphs
                let ru = request.subgraph_request.uri_mut();

                // let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

                // Call the asynchronous connect method using the runtime.
                let config = executor::block_on(get_cached_config("1".to_string())).unwrap();
                *ru = config.partner_graph_url.parse::<Uri>().unwrap();

                println!("{}", request.subgraph_request.uri());
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
