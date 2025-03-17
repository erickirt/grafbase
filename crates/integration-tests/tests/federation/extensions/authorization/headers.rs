use engine::{Engine, ErrorResponse};
use graphql_mocks::{EchoSchema, Schema};
use integration_tests::{
    federation::{EngineExt, TestExtension},
    runtime,
};
use runtime::{
    extension::{AuthorizationDecisions, QueryElement, TokenRef},
    hooks::DynHookContext,
};

use crate::federation::extensions::{
    authentication::{AuthenticationExt, static_token::StaticToken},
    authorization::AuthorizationExt,
};

#[derive(Default)]
pub(super) struct InsertTokenAsHeader;

#[async_trait::async_trait]
impl TestExtension for InsertTokenAsHeader {
    #[allow(clippy::manual_async_fn)]
    async fn authorize_query(
        &self,
        _wasm_context: &DynHookContext,
        headers: &tokio::sync::RwLock<http::HeaderMap>,
        token: TokenRef<'_>,
        _elements_grouped_by_directive_name: Vec<(&str, Vec<QueryElement<'_, serde_json::Value>>)>,
    ) -> Result<AuthorizationDecisions, ErrorResponse> {
        headers.write().await.insert(
            "token",
            http::HeaderValue::from_bytes(token.as_bytes().unwrap_or_default()).unwrap(),
        );
        Ok(AuthorizationDecisions::GrantAll)
    }
}

#[test]
fn can_inject_token_into_headers() {
    let response = runtime().block_on(async move {
        let engine = Engine::builder()
            .with_subgraph(EchoSchema.with_sdl(
                r#"
                extend schema @link(url: "authorization-1.0.0", import: ["@auth"])

                type Query {
                    headers: [Header!]! @auth
                }
                type Header {
                    name: String!
                    value: String!
                }
                "#,
            ))
            .with_extension(AuthenticationExt::new(StaticToken::bytes("Hello world!".into())))
            .with_extension(AuthorizationExt::new(InsertTokenAsHeader))
            .with_toml_config(
                r#"
            [[authentication.providers]]

            [authentication.providers.extension]
            extension = "authentication"
            "#,
            )
            .build()
            .await;

        engine.post("query { headers { name value }}").await
    });

    insta::assert_json_snapshot!(response,  @r#"
    {
      "data": {
        "headers": [
          {
            "name": "accept",
            "value": "application/graphql-response+json; charset=utf-8, application/json; charset=utf-8"
          },
          {
            "name": "accept-encoding",
            "value": "gzip, br, deflate"
          },
          {
            "name": "content-length",
            "value": "59"
          },
          {
            "name": "content-type",
            "value": "application/json"
          },
          {
            "name": "token",
            "value": "Hello world!"
          }
        ]
      }
    }
    "#);
}
