use integration_tests::{federation::Gateway, runtime};

use super::EchoExt;

#[test]
fn missing_nullable_field() {
    runtime().block_on(async move {
        let engine = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo-1.0.0", import: ["@echo", "@meta"])
                    @meta(input: {})

                scalar JSON

                type Query {
                    echo: JSON @echo(input: {})
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(
                r#"
                directive @meta(input: EchoInput!) on SCHEMA
                directive @echo(input: EchoInput!) on FIELD_DEFINITION

                input EchoInput {
                    value: String
                }
                "#,
            ))
            .build()
            .await;

        let response = engine.post("query { echo }").await;
        insta::assert_json_snapshot!(response, @r#"
        {
          "data": {
            "echo": {
              "schema": {
                "meta": {
                  "input": {}
                }
              },
              "directive": {
                "input": {}
              },
              "input": {}
            }
          }
        }
        "#);
    });
}

#[test]
fn missing_required_field() {
    runtime().block_on(async move {
        // Invalid field directive
        let result = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo-1.0.0", import: ["@echo", "@meta"])

                scalar JSON

                type Query {
                    echo: JSON @echo(input: {})
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(r#"
                directive @meta(input: EchoInput!) on SCHEMA
                directive @echo(input: EchoInput!) on FIELD_DEFINITION

                input EchoInput {
                    value: String!
                }
            "#))
            .try_build()
            .await;

        insta::assert_debug_snapshot!(result.err(), @r#"
        Some(
            "At Query.echo for the extension 'echo-1.0.0' directive @echo: Found a null where we expected a String! at path '.input.value'",
        )
        "#);

        // Invalid schema directive
        let result = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo-1.0.0", import: ["@echo", "@meta"])
                    @meta(input: {})

                scalar JSON

                type Query {
                    echo: JSON
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(r#"
                directive @meta(input: EchoInput!) on SCHEMA
                directive @echo(input: EchoInput!) on FIELD_DEFINITION

                input EchoInput {
                    value: String!
                }
            "#))
            .try_build()
            .await;

        insta::assert_debug_snapshot!(result.err(), @r#"
        Some(
            "At subgraph named 'a' for the extension 'echo-1.0.0' directive @meta: Found a null where we expected a String! at path '.input.value'",
        )
        "#);
    });
}

#[test]
fn too_many_fields() {
    runtime().block_on(async move {
        // Invalid field directive
        let result = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo-1.0.0", import: ["@echo", "@meta"])

                scalar JSON

                type Query {
                    echo: JSON @echo(input: { value: "test", other: 1 })
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(r#"
                directive @meta(input: EchoInput!) on SCHEMA
                directive @echo(input: EchoInput!) on FIELD_DEFINITION

                input EchoInput {
                    value: String
                }
            "#))
            .try_build()
            .await;

        insta::assert_debug_snapshot!(result.err(), @r#"
        Some(
            "At Query.echo for the extension 'echo-1.0.0' directive @echo: Input object EchoInput does not have a field named 'other' at path '.input'",
        )
        "#);

        // Invalid schema directive
        let result = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo-1.0.0", import: ["@echo", "@meta"])
                    @meta(input: { value: "test", other: 1 })

                scalar JSON

                type Query {
                    echo: JSON
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(r#"
                directive @meta(input: EchoInput!) on SCHEMA
                directive @echo(input: EchoInput!) on FIELD_DEFINITION

                input EchoInput {
                    value: String
                }
            "#))
            .try_build()
            .await;

        insta::assert_debug_snapshot!(result.err(), @r#"
        Some(
            "At subgraph named 'a' for the extension 'echo-1.0.0' directive @meta: Input object EchoInput does not have a field named 'other' at path '.input'",
        )
        "#);
    });
}

#[test]
fn not_an_object() {
    runtime().block_on(async move {
        // Invalid field directive
        let result = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo-1.0.0", import: ["@echo", "@meta"])

                scalar JSON

                type Query {
                    echo: JSON @echo(input: [])
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(r#"
                directive @meta(input: EchoInput!) on SCHEMA
                directive @echo(input: EchoInput!) on FIELD_DEFINITION

                input EchoInput {
                    value: String
                }
            "#))
            .try_build()
            .await;

        insta::assert_debug_snapshot!(result.err(), @r#"
        Some(
            "At Query.echo for the extension 'echo-1.0.0' directive @echo: Found a List value where we expected a 'EchoInput' input object at path '.input'",
        )
        "#);

        // Invalid schema directive
        let result = Gateway::builder()
            .with_subgraph_sdl(
                "a",
                r#"
                extend schema
                    @link(url: "echo-1.0.0", import: ["@echo", "@meta"])
                    @meta(input: [])

                scalar JSON

                type Query {
                    echo: JSON
                }
                "#,
            )
            .with_extension(EchoExt::with_sdl(r#"
                directive @meta(input: EchoInput!) on SCHEMA
                directive @echo(input: EchoInput!) on FIELD_DEFINITION

                input EchoInput {
                    value: String
                }
            "#))
            .try_build()
            .await;

        insta::assert_debug_snapshot!(result.err(), @r#"
        Some(
            "At subgraph named 'a' for the extension 'echo-1.0.0' directive @meta: Found a List value where we expected a 'EchoInput' input object at path '.input'",
        )
        "#);
    });
}
