use std::sync::Arc;

use engine::{Engine, ErrorResponse, GraphqlError};
use engine_schema::DirectiveSite;
use graphql_mocks::dynamic::DynamicSchema;
use integration_tests::{
    federation::{EngineExt, TestExtension},
    runtime,
};
use runtime::{
    extension::{AuthorizationDecisions, QueryElement},
    hooks::DynHookContext,
};

use crate::federation::extensions::authorization::AuthorizationExt;

#[derive(Default)]
pub(super) struct GrantAll;

#[async_trait::async_trait]
impl TestExtension for GrantAll {
    #[allow(clippy::manual_async_fn)]
    async fn authorize_query(
        &self,
        _ctx: Arc<engine::RequestContext>,
        _: &DynHookContext,
        _elements_grouped_by_directive_name: Vec<(&str, Vec<QueryElement<'_, serde_json::Value>>)>,
    ) -> Result<AuthorizationDecisions, ErrorResponse> {
        Ok(AuthorizationDecisions::GrantAll)
    }

    async fn authorize_response(
        &self,
        _ctx: Arc<engine::RequestContext>,
        _wasm_context: &DynHookContext,
        _directive_name: &str,
        _directive_site: DirectiveSite<'_>,
        _items: Vec<serde_json::Value>,
    ) -> Result<AuthorizationDecisions, GraphqlError> {
        Ok(AuthorizationDecisions::GrantAll)
    }
}

#[test]
fn can_grant_all() {
    runtime().block_on(async move {
        let engine = Engine::builder()
            .with_subgraph(
                DynamicSchema::builder(
                    r#"
                extend schema @link(url: "authorization-1.0.0", import: ["@auth"])

                type Query {
                    greeting: String @auth
                    forbidden: String @auth
                }
                "#,
                )
                .with_resolver("Query", "forbidden", serde_json::Value::String("Oh no!".to_owned()))
                .with_resolver("Query", "greeting", serde_json::Value::String("Hi!".to_owned()))
                .into_subgraph("x"),
            )
            .with_extension(AuthorizationExt::new(GrantAll))
            .build()
            .await;

        let response = engine.post("query { greeting forbidden }").await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "greeting": "Hi!",
            "forbidden": "Oh no!"
          }
        }
        "#);

        let sent = engine.drain_graphql_requests_sent_to_by_name("x");
        insta::assert_json_snapshot!(sent, @r#"
        [
          {
            "query": "query { greeting forbidden }",
            "operationName": null,
            "variables": {},
            "extensions": {}
          }
        ]
        "#)
    });
}
