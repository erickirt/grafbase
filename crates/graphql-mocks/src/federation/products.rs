use std::sync::Arc;

// See https://github.com/async-graphql/examples
use async_graphql::{ComplexObject, Context, EmptyMutation, Object, Schema, SimpleObject};
use futures::Stream;

use crate::websockets::ConnectionInitPayload;

pub struct FederatedProductsSchema {
    schema: Schema<Query, EmptyMutation, Subscription>,
}

impl crate::Subgraph for FederatedProductsSchema {
    fn name(&self) -> String {
        "products".to_string()
    }
    async fn start(self) -> crate::MockGraphQlServer {
        crate::MockGraphQlServer::new(self).await
    }
}

impl Default for FederatedProductsSchema {
    fn default() -> Self {
        let products = vec![
            Product {
                upc: "top-1".to_string(),
                name: "Trilby".to_string(),
                price: 11,
                weight_grams: 100,
            },
            Product {
                upc: "top-2".to_string(),
                name: "Fedora".to_string(),
                price: 22,
                weight_grams: 200,
            },
            Product {
                upc: "top-3".to_string(),
                name: "Boater".to_string(),
                price: 33,
                weight_grams: 300,
            },
            Product {
                upc: "top-4".to_string(),
                name: "Jeans".to_string(),
                price: 44,
                weight_grams: 400,
            },
            Product {
                upc: "top-5".to_string(),
                name: "Pink Jeans".to_string(),
                price: 55,
                weight_grams: 500,
            },
        ];
        let schema = Schema::build(Query, EmptyMutation, Subscription)
            .enable_federation()
            .enable_subscription_in_federation()
            .data(products)
            .finish();
        Self { schema }
    }
}

#[async_trait::async_trait]
impl super::super::Schema for FederatedProductsSchema {
    async fn execute(
        &self,
        _headers: Vec<(String, String)>,
        request: async_graphql::Request,
    ) -> async_graphql::Response {
        self.schema.execute(request).await
    }

    fn execute_stream(
        &self,
        request: async_graphql::Request,
        session_data: Option<Arc<async_graphql::Data>>,
    ) -> futures::stream::BoxStream<'static, async_graphql::Response> {
        async_graphql::Executor::execute_stream(&self.schema, request, session_data)
    }

    fn sdl(&self) -> String {
        self.schema
            .sdl_with_options(async_graphql::SDLExportOptions::new().federation())
    }
}

#[derive(async_graphql::Enum, Clone, Copy, PartialEq, Eq)]
pub enum WeightUnit {
    Kilogram,
    Gram,
}

#[derive(Clone, SimpleObject)]
#[graphql(complex)]
struct Product {
    upc: String,
    name: String,
    #[graphql(shareable)]
    price: i32,
    #[graphql(skip)]
    weight_grams: u64,
}

#[ComplexObject]
impl Product {
    async fn weight(&self, unit: WeightUnit) -> f64 {
        match unit {
            WeightUnit::Kilogram => (self.weight_grams as f64) / 1000.0,
            WeightUnit::Gram => self.weight_grams as f64,
        }
    }
}

struct Query;

#[Object]
impl Query {
    async fn top_products<'a>(&self, ctx: &'a Context<'_>) -> &'a Vec<Product> {
        ctx.data_unchecked::<Vec<Product>>()
    }

    async fn product<'a>(&self, ctx: &'a Context<'_>, upc: String) -> Option<&'a Product> {
        let products = ctx.data_unchecked::<Vec<Product>>();
        products.iter().find(|product| product.upc == upc)
    }

    #[graphql(entity)]
    async fn find_product_by_upc<'a>(&self, ctx: &'a Context<'_>, upc: String) -> Option<&'a Product> {
        let products = ctx.data_unchecked::<Vec<Product>>();
        products.iter().find(|product| product.upc == upc)
    }

    #[graphql(entity)]
    async fn find_product_by_name<'a>(&self, ctx: &'a Context<'_>, name: String) -> Option<&'a Product> {
        let products = ctx.data_unchecked::<Vec<Product>>();
        products.iter().find(|product| product.name == name)
    }
}

struct Subscription;

#[async_graphql::Subscription]
impl Subscription {
    async fn new_products(&self, ctx: &Context<'_>) -> impl Stream<Item = Product> {
        futures::stream::iter(
            ctx.data_unchecked::<Vec<Product>>()
                .iter()
                .filter(|product| product.upc == "top-4" || product.upc == "top-5")
                .cloned()
                .collect::<Vec<Product>>(),
        )
    }

    async fn connection_init_payload(&self, ctx: &Context<'_>) -> impl Stream<Item = Option<serde_json::Value>> {
        let payload = ctx.data_unchecked::<ConnectionInitPayload>().0.clone();

        futures::stream::once(std::future::ready(if payload.is_null() { None } else { Some(payload) }))
    }

    async fn http_header(&self, ctx: &Context<'_>, name: Vec<String>) -> impl Stream<Item = serde_json::Value> {
        let headers = ctx.data_unchecked::<http::HeaderMap>().clone();
        futures::stream::iter(name.into_iter().map(move |name| {
            let value = headers
                .get(&name)
                .map(|value| serde_json::Value::String(String::from_utf8_lossy(value.as_bytes()).into()))
                .unwrap_or_default();
            serde_json::json!({
                "name": name,
                "value": value,
            })
        }))
    }
}
