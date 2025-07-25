use futures::Future;
use graphql_mocks::SecureSchema;
use integration_tests::{
    gateway::Gateway,
    openid::{CoreClientExt, JWKS_URI, OryHydraOpenIDProvider},
    runtime,
};

pub(super) fn with_secure_schema<F, O>(f: impl FnOnce(Gateway) -> F) -> O
where
    F: Future<Output = O>,
{
    runtime().block_on(async move {
        let config = indoc::formatdoc! {r#"
            [[authentication.providers]]

            [authentication.providers.jwt]
            name = "my-jwt"

            [authentication.providers.jwt.jwks]
            url = "{JWKS_URI}"

            [[authentication.providers]]

            [authentication.providers.anonymous]
        "#};

        let engine = Gateway::builder()
            .with_subgraph(SecureSchema::default())
            .with_toml_config(config)
            .build()
            .await;

        f(engine).await
    })
}

#[test]
fn authenticated() {
    with_secure_schema(|engine| async move {
        let token = OryHydraOpenIDProvider::default()
            .create_client()
            .await
            .get_access_token_with_client_credentials(&[])
            .await;

        let response = engine
            .post("query { check { anonymous } }")
            .header("Authorization", format!("Bearer {token}"))
            .await;
        insta::assert_json_snapshot!(response, @r###"
        {
          "data": {
            "check": {
              "anonymous": "Hello anonymous!"
            }
          }
        }
        "###);

        let response = engine
            .post("query { check { mustBeAuthenticated } }")
            .header("Authorization", format!("Bearer {token}"))
            .await;
        insta::assert_json_snapshot!(response, @r###"
        {
          "data": {
            "check": {
              "mustBeAuthenticated": "You are authenticated"
            }
          }
        }
        "###);
    });
}

#[test]
fn not_authenticated() {
    with_secure_schema(|engine| async move {
        let response = engine.post("query { check { anonymous } }").await;
        insta::assert_json_snapshot!(response, @r###"
        {
          "data": {
            "check": {
              "anonymous": "Hello anonymous!"
            }
          }
        }
        "###);

        let response = engine.post("query { check { mustBeAuthenticated } }").await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": null,
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 1,
                  "column": 17
                }
              ],
              "path": [
                "check",
                "mustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);

        // We shouldn't have requested the field.
        let requests = engine.drain_graphql_requests_sent_to::<SecureSchema>();
        insta::assert_json_snapshot!(requests, @r#"
        [
          {
            "query": "query { check { anonymous } }",
            "operationName": null,
            "variables": {},
            "extensions": {}
          },
          {
            "query": "query { check { __typename @skip(if: true) } }",
            "operationName": null,
            "variables": {},
            "extensions": {}
          }
        ]
        "#);
    });
}

#[test]
fn faillible_authenticated() {
    with_secure_schema(|engine| async move {
        let response = engine
            .post("query { check { anonymous faillibleMustBeAuthenticated } }")
            .await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "check": {
              "anonymous": "Hello anonymous!",
              "faillibleMustBeAuthenticated": null
            }
          },
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 1,
                  "column": 27
                }
              ],
              "path": [
                "check",
                "faillibleMustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);
    });
}

#[test]
fn authenticated_on_nullable_field() {
    with_secure_schema(|engine| async move {
        let response = engine.post("query { nullableCheck { mustBeAuthenticated } }").await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "nullableCheck": null
          },
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 1,
                  "column": 25
                }
              ],
              "path": [
                "nullableCheck",
                "mustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);
    });
}

#[test]
fn authenticated_on_union() {
    with_secure_schema(|engine| async move {
        let response = engine
            .post("query { entity(check: false) { __typename ... on Check { mustBeAuthenticated } } }")
            .await;

        insta::assert_json_snapshot!(response, @r###"
        {
          "data": {
            "entity": {
              "__typename": "User"
            }
          }
        }
        "###);

        let response = engine
            .post("query { entity(check: true) { __typename ... on Check { mustBeAuthenticated } } }")
            .await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": null,
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 1,
                  "column": 57
                }
              ],
              "path": [
                "entity",
                "mustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);

        let response = engine
            .post("query { entity(check: true) { __typename ... on Check { faillibleMustBeAuthenticated } } }")
            .await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "entity": {
              "__typename": "Check",
              "faillibleMustBeAuthenticated": null
            }
          },
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 1,
                  "column": 57
                }
              ],
              "path": [
                "entity",
                "faillibleMustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);
    });
}

#[test]
fn authenticated_on_list_with_nullable_items() {
    with_secure_schema(|engine| async move {
        let response = engine
            .post(
                r###"
                query {
                    entitiesNullable(check: false) {
                        __typename
                        ... on Check { mustBeAuthenticated }
                        ... on User { name }
                    }
                }
                "###,
            )
            .await;
        insta::assert_json_snapshot!(response, @r###"
        {
          "data": {
            "entitiesNullable": [
              {
                "__typename": "User",
                "name": "rusty"
              }
            ]
          }
        }
        "###);

        let response = engine
            .post(
                r###"
                query {
                    entitiesNullable(check: true) {
                        __typename
                        ... on Check { mustBeAuthenticated }
                        ... on User { name }
                    }
                }
                "###,
            )
            .await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "entitiesNullable": [
              {
                "__typename": "User",
                "name": "rusty"
              },
              null
            ]
          },
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 5,
                  "column": 40
                }
              ],
              "path": [
                "entitiesNullable",
                1,
                "mustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);

        let response = engine
            .post(
                r###"
                query {
                    entitiesNullable(check: true) {
                        __typename
                        ... on Check { faillibleMustBeAuthenticated }
                        ... on User { name }
                    }
                }
                "###,
            )
            .await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "entitiesNullable": [
              {
                "__typename": "User",
                "name": "rusty"
              },
              {
                "__typename": "Check",
                "faillibleMustBeAuthenticated": null
              }
            ]
          },
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 5,
                  "column": 40
                }
              ],
              "path": [
                "entitiesNullable",
                1,
                "faillibleMustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);
    });
}

#[test]
fn authenticated_on_list_with_required_items() {
    with_secure_schema(|engine| async move {
        let response = engine
            .post(
                r###"
                query {
                    entities(check: false) {
                        __typename
                        ... on Check { mustBeAuthenticated }
                        ... on User { name }
                    }
                }
                "###,
            )
            .await;
        insta::assert_json_snapshot!(response, @r###"
        {
          "data": {
            "entities": [
              {
                "__typename": "User",
                "name": "rusty"
              }
            ]
          }
        }
        "###);

        let response = engine
            .post(
                r###"
                query {
                    entities(check: true) {
                        __typename
                        ... on Check { mustBeAuthenticated }
                        ... on User { name }
                    }
                }
                "###,
            )
            .await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": null,
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 5,
                  "column": 40
                }
              ],
              "path": [
                "entities",
                1,
                "mustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);

        let response = engine
            .post(
                r###"
                query {
                    entities(check: true) {
                        __typename
                        ... on Check { faillibleMustBeAuthenticated }
                        ... on User { name }
                    }
                }
                "###,
            )
            .await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "entities": [
              {
                "__typename": "User",
                "name": "rusty"
              },
              {
                "__typename": "Check",
                "faillibleMustBeAuthenticated": null
              }
            ]
          },
          "errors": [
            {
              "message": "Unauthenticated",
              "locations": [
                {
                  "line": 5,
                  "column": 40
                }
              ],
              "path": [
                "entities",
                1,
                "faillibleMustBeAuthenticated"
              ],
              "extensions": {
                "code": "UNAUTHENTICATED"
              }
            }
          ]
        }
        "#);
    });
}
