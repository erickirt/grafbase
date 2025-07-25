use std::sync::Arc;

// See https://github.com/async-graphql/examples
use async_graphql::{ComplexObject, Context, EmptyMutation, EmptySubscription, Enum, ID, Object, Schema, SimpleObject};

pub struct FederatedReviewsSchema {
    schema: Schema<Query, EmptyMutation, EmptySubscription>,
}

impl crate::Subgraph for FederatedReviewsSchema {
    fn name(&self) -> String {
        "reviews".to_string()
    }
    async fn start(self) -> crate::MockGraphQlServer {
        crate::MockGraphQlServer::new(self).await
    }
}

impl Default for FederatedReviewsSchema {
    fn default() -> Self {
        let reviews =
            vec![
            Review {
                id: "review-1".into(),
                author_id: Some("1234".into()),
                body: "A highly effective form of birth control.".into(),
                pictures: vec![
                    Picture {
                        url: "http://localhost:8080/ugly_hat.jpg".to_string(),
                        width: 100,
                        height: 100,
                        alt_text: "A Trilby".to_string(),
                    },
                    Picture {
                        url: "http://localhost:8080/troll_face.jpg".to_string(),
                        width: 42,
                        height: 42,
                        alt_text: "The troll face meme".to_string(),
                    },
                ],
                product: Product {
                    upc: "top-1".to_string(),
                    price: 10,
                },
            },
            Review {
                id: "review-2".into(),
                author_id: Some("1234".into()),
                body:
                    "Fedoras are one of the most fashionable hats around and can look great with a variety of outfits."
                        .into(),
                pictures: vec![],
                product: Product {
                    upc: "top-2".to_string(),
                    price: 20,
                },
            },
            Review {
                id: "review-3".into(),
                author_id: Some("7777".into()),
                body: "This is the last straw. Hat you will wear. 11/10".into(),
                pictures: vec![],
                product: Product {
                    upc: "top-3".to_string(),
                    price: 30,
                },
            },
            Review {
                id: "review-5".into(),
                author_id: None,
                body: "Beautiful Pink, my parrot loves it. Definitely recommend!".into(),
                pictures: vec![],
                product: Product { upc: "top-5".into(), price: 50 }
            },
        ];

        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .data(reviews)
            .enable_federation()
            .finish();
        Self { schema }
    }
}

#[async_trait::async_trait]
impl super::super::Schema for FederatedReviewsSchema {
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

#[derive(SimpleObject)]
#[graphql(complex)]
struct User {
    id: ID,
    #[graphql(override_from = "accounts")]
    review_count: u32,
    #[graphql(external)]
    joined_timestamp: u64,
}

#[derive(Enum, Eq, PartialEq, Copy, Clone)]
#[allow(clippy::enum_variant_names)]
enum Trustworthiness {
    ReallyTrusted,
    KindaTrusted,
    NotTrusted,
}

#[ComplexObject]
impl User {
    async fn reviews<'a>(&self, ctx: &'a Context<'_>) -> Vec<&'a Review> {
        let reviews = ctx.data_unchecked::<Vec<Review>>();
        let maybe_id = Some(self.id.clone());
        reviews.iter().filter(|review| review.author_id == maybe_id).collect()
    }

    #[graphql(requires = "joinedTimestamp")]
    async fn trustworthiness(&self) -> Trustworthiness {
        if self.joined_timestamp < 1_000 && self.review_count > 1 {
            Trustworthiness::ReallyTrusted
        } else if self.joined_timestamp < 2_000 {
            Trustworthiness::KindaTrusted
        } else {
            Trustworthiness::NotTrusted
        }
    }
}

#[derive(SimpleObject)]
#[graphql(complex)]
struct Product {
    upc: String,
    #[graphql(external)]
    price: i32,
}

#[ComplexObject]
impl Product {
    async fn reviews<'a>(&self, ctx: &'a Context<'_>) -> Vec<&'a Review> {
        let reviews = ctx.data_unchecked::<Vec<Review>>();
        reviews.iter().filter(|review| review.product.upc == self.upc).collect()
    }
}

#[derive(SimpleObject)]
#[graphql(complex)]
struct Review {
    id: ID,
    body: String,
    pictures: Vec<Picture>,
    #[graphql(skip)]
    author_id: Option<ID>,
    #[graphql(provides = "price")]
    product: Product,
}

#[ComplexObject]
impl Review {
    async fn author(&self) -> Option<User> {
        self.author_id.as_ref().map(|user_id| user_by_id(user_id.clone(), None))
    }
}

#[derive(SimpleObject)]
#[graphql(shareable)]
struct Picture {
    url: String,
    width: u32,
    height: u32,
    #[graphql(inaccessible)] // Field not added to Accounts yet
    alt_text: String,
}

#[derive(SimpleObject)]
#[graphql(complex, interface_object)]
struct ShippingService {
    id: String,
    #[graphql(external)]
    name: String,
}

#[ComplexObject]
impl ShippingService {
    #[graphql(requires = "name")]
    async fn reviews(&self) -> Vec<ShippingServiceReview> {
        match self.name.as_str() {
            "Planet Express" => vec![ShippingServiceReview {
                body: "Not as good as Mom's Friendly Delivery Company".to_string(),
            }],
            "Cher Ami" => vec![ShippingServiceReview {
                body: "Saved my life in the war".to_string(),
            }],
            _ => Vec::new(),
        }
    }
}

#[derive(SimpleObject)]
struct ShippingServiceReview {
    body: String,
}

struct Query;

#[Object]
impl Query {
    #[graphql(entity)]
    async fn find_user_by_id(&self, #[graphql(key)] id: ID, joined_timestamp: Option<u64>) -> User {
        user_by_id(id, joined_timestamp)
    }

    #[graphql(entity)]
    async fn find_product_by_upc(&self, upc: String) -> Product {
        Product { upc, price: 0 }
    }

    #[graphql(entity)]
    async fn shipping_service_by_id(&self, #[graphql(key)] id: String, name: Option<String>) -> ShippingService {
        ShippingService {
            id,
            name: name.unwrap_or_default(),
        }
    }
}

fn user_by_id(id: ID, joined_timestamp: Option<u64>) -> User {
    let review_count = match id.as_str() {
        "1234" => 2,
        "7777" => 1,
        _ => 0,
    };
    // This will be set if the user requested the fields that require it.
    let joined_timestamp = joined_timestamp.unwrap_or(9001);
    User {
        id,
        review_count,
        joined_timestamp,
    }
}
