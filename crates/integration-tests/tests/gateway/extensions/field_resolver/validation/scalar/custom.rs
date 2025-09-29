use integration_tests::{gateway::Gateway, runtime};

use super::EchoExt;

#[test]
fn valid_string() {
    let response = runtime().block_on(async move {
        let engine = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo", import: ["@echo", "@meta"])
                    @meta(value: [{a:1}, 789, "test"])

                scalar JSON

                type Query {
                    echo: JSON @echo(value: {x: 7.123, y: null, z: false, w: VALUE})
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(
                r#"
                directive @meta(value: Any!) on SCHEMA
                directive @echo(value: Any!) on FIELD_DEFINITION

                scalar Any
            "#,
            ))
            .build()
            .await;

        engine.post("query { echo }").await
    });

    insta::assert_json_snapshot!(response, @r#"
    {
      "data": {
        "echo": {
          "schema": {
            "meta": {
              "value": [
                {
                  "a": 1
                },
                789,
                "test"
              ]
            }
          },
          "directive": {
            "value": {
              "x": 7.123,
              "y": null,
              "z": false,
              "w": "VALUE"
            }
          },
          "input": {}
        }
      }
    }
    "#);
}
