use super::{ResolvedValue, ResolverContext, ResolverTrait};
use crate::registry::utils::value_to_attribute;
use crate::registry::variables::VariableResolveDefinition;
use crate::{Context, Error, Value};
use chrono::Utc;
use dynamodb::{BatchGetItemLoaderError, DynamoDBBatchersData, DynamoDBContext, TxItem};
use dynomite::dynamodb::{Delete, Put, TransactWriteItem};
use dynomite::{Attribute, AttributeValue};
use futures_util::FutureExt;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;

#[non_exhaustive]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, Hash)]
pub enum DynamoMutationResolver {
    /// Create a new Node
    ///
    /// We do create a new node and store the generated ID into the ResolverContext to allow a
    /// ContextData Resolver to access this id if needed.
    /// When we create a Node with Edges, we fetch those edges before creating the Node and the
    /// vertices.
    ///
    /// # Flow
    ///
    /// -> Generate the ID of the new Node
    /// -> Fetch the Edges needed.
    /// -> Store the Node
    /// -> Store the Vertices.
    ///
    /// # Returns
    ///
    /// This resolver return a Value like this:
    ///
    /// ```json
    /// {
    ///   "id": "<generated_id>"
    /// }
    /// ```
    CreateNode {
        input: VariableResolveDefinition,
        /// Type defined for GraphQL side, it's used to be able to know if we manipulate a Node
        /// and if this Node got Edges. This type must the the Type visible on the GraphQL Schema.
        ty: String,
    },
    /// The delete Node will delete the node and the relation to the associated edges, as
    /// an edge is a Node, we won't have any unreachable node in our Database.
    ///
    /// We also store the deleted ID into the ResolverContext to allow a ContextData Resolver to
    /// access this id if needed.
    ///
    /// # Example
    ///
    /// A node with two edges:
    ///
    /// ```ignore
    ///                     ┌────────┐
    ///                 ┌───┤ Edge 1 │
    ///                 │   └────────┘
    ///      ┌────┐     │
    ///      │Node├─────┤
    ///      └────┘     │
    ///                 │   ┌────────┐
    ///                 └───┤ Edge 2 │
    ///                     └────────┘
    /// ```
    ///
    /// When we delete this node, we'll update the graph to become:
    ///
    /// ```ignore
    ///                     ┌────────┐
    ///                     │ Edge 1 │
    ///                     └────────┘
    ///                     
    ///
    ///
    ///                     ┌────────┐
    ///                     │ Edge 2 │
    ///                     └────────┘
    /// ```
    ///
    /// And as every edges of a Node are a Node too, they are still reachable.
    ///
    /// TODO: Right now, we delete only the main node and not the vertices.
    ///
    /// In the future, when we'll have worked on an async process to optimize we'll be able to
    /// optimize the delete operation:
    ///
    /// In fact it's useless to delete the vertices between the node when you do not have a
    /// bi-directional relaton between nodes. You could only remove the node and have an async
    /// process remove the vertices as soon as possible. It woulnd't affect the future user's
    /// queries but would allow a deletion to be executed with a constant time of one operation.
    ///
    /// # Returns
    ///
    /// This resolver return a Value like this:
    ///
    /// ```json
    /// {
    ///   "id": "<deleted_id>"
    /// }
    /// ```
    DeleteNode { id: VariableResolveDefinition },
}

#[async_trait::async_trait]
impl ResolverTrait for DynamoMutationResolver {
    async fn resolve(
        &self,
        ctx: &Context<'_>,
        resolver_ctx: &ResolverContext<'_>,
        last_resolver_value: Option<&ResolvedValue>,
    ) -> Result<ResolvedValue, Error> {
        let batchers = &ctx.data::<DynamoDBBatchersData>()?;
        let transaction_batcher = &batchers.transaction;
        let loader_batcher = &batchers.loader;
        let dynamodb_ctx = ctx.data::<DynamoDBContext>()?;

        match self {
            // This one is tricky, when we create a new node, we have to check that the node do not
            // contains any Edges on the first level. If there is an edge at the first level we
            // need to fetch this edge as a node and store it alongside the actual node.
            //
            // Why?
            //
            // Because it's how we store the data.
            DynamoMutationResolver::CreateNode { input, ty } => {
                let ctx_ty = ctx.registry().types.get(ty).ok_or_else(|| {
                    Error::new("Internal Error: Failed process the associated schema.")
                })?;
                let edges = ctx_ty.edges();
                let edges_len = edges.len();
                let autogenerated_id = format!("{}#{}", ty, resolver_ctx.execution_id,);

                let input = match input
                    .param(ctx, last_resolver_value.map(|x| x.data_resolved.borrow()))?
                    .expect("can't fail")
                {
                    Value::Object(inner) => inner,
                    _ => {
                        return Err(Error::new("Internal Error: failed to infer key"));
                    }
                };

                // We do create the futures to be run to have the edges.
                let edges: Vec<
                    Pin<
                        Box<
                            dyn Future<
                                    Output = Result<
                                        (
                                            String,
                                            HashMap<
                                                (String, String),
                                                HashMap<String, AttributeValue>,
                                            >,
                                        ),
                                        BatchGetItemLoaderError,
                                    >,
                                > + Send,
                        >,
                    >,
                > = edges
                    .into_iter()
                    .map(|(field, _)| {
                        // If it's an edge, we can only have an ID or an Array of ID
                        // as it's how we modelized the relations.
                        // Or it can also be null.
                        let field_value = input.get(field).and_then(|value| match value {
                            Value::String(inner) => Some(vec![(inner.clone(), inner.clone())]),
                            Value::List(list) => Some(
                                list.iter()
                                    .map(|value| match value {
                                        Value::String(inner) => (inner.clone(), inner.clone()),
                                        _ => panic!(),
                                    })
                                    .collect(),
                            ),
                            _ => None,
                        });

                        match field_value {
                            Some(field_value) => {
                                let fetch_edge_id_future: Pin<
                                    Box<
                                        dyn Future<
                                                Output = Result<
                                                    (
                                                        String,
                                                        HashMap<
                                                            (String, String),
                                                            HashMap<String, AttributeValue>,
                                                        >,
                                                    ),
                                                    BatchGetItemLoaderError,
                                                >,
                                            > + Send,
                                    >,
                                > = Box::pin(
                                    loader_batcher
                                        .load_many(field_value)
                                        .map(|x| x.map(|r| (field.to_owned(), r))),
                                );
                                fetch_edge_id_future
                            }
                            None => Box::pin(async move { Ok((field.to_owned(), HashMap::new())) }),
                        }
                    })
                    .collect();

                // We run them to fetch the edges.
                let edges = futures_util::future::try_join_all(edges)
                    .await?
                    .into_iter()
                    .fold(
                        HashMap::with_capacity(edges_len),
                        |mut acc, (field, value)| {
                            acc.insert(field, value.into_values().collect::<Vec<_>>());
                            acc
                        },
                    );

                let mut item = input
                    .into_iter()
                    .fold(HashMap::new(), |mut acc, (key, val)| {
                        let key = key.to_string();
                        if !edges.contains_key(&key) {
                            acc.insert(
                                key,
                                value_to_attribute(val.into_json().expect("can't fail")),
                            );
                        }
                        acc
                    });

                let autogenerated_id_attr = autogenerated_id.clone().into_attr();

                item.insert("__pk".to_string(), autogenerated_id_attr.clone());
                item.insert("__sk".to_string(), autogenerated_id_attr.clone());
                item.insert("__type".to_string(), ty.clone().into_attr());

                item.insert("created_at".to_string(), Utc::now().to_string().into_attr());
                item.insert("updated_at".to_string(), Utc::now().to_string().into_attr());

                item.insert("__gsi1pk".to_string(), ty.clone().into_attr());
                item.insert("__gsi1sk".to_string(), autogenerated_id_attr.clone());

                item.insert("__gsi2pk".to_string(), autogenerated_id_attr.clone());
                item.insert("__gsi2sk".to_string(), autogenerated_id_attr.clone());

                let t = TxItem {
                    pk: autogenerated_id.clone(),
                    sk: autogenerated_id.clone(),
                    transaction: TransactWriteItem {
                        put: Some(Put {
                            table_name: dynamodb_ctx.dynamodb_table_name.clone(),
                            item,
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                };

                let mut transactions = Vec::with_capacity(edges_len + 1);
                transactions.push(t);

                let ty_attr = ty.clone().into_attr();
                // We store the vertices.
                for (_, edge) in edges {
                    for mut inner_edge in edge {
                        inner_edge.insert("__gsi1pk".to_string(), ty_attr.clone());
                        // We do store the PK into the GSISK to allow us to group edges based on
                        // their node.
                        // TODO: Explain in a md how and why the data is stored the way it's
                        // stored.
                        inner_edge.insert("__gsi1sk".to_string(), autogenerated_id_attr.clone());

                        // We do replace the PK by the Node's PK.
                        inner_edge.insert("__pk".to_string(), autogenerated_id_attr.clone());
                        // The GSI2 is an inversed index, so we update the SK too.
                        inner_edge.insert("__gsi2sk".to_string(), autogenerated_id_attr.clone());

                        let sk = inner_edge
                            .get("__sk")
                            .expect("Can't fail, it's the sorting key.")
                            .s
                            .clone()
                            .expect("Can't fail, the sorting key is a String.");

                        transactions.push(TxItem {
                            pk: autogenerated_id.clone(),
                            sk,
                            transaction: TransactWriteItem {
                                put: Some(Put {
                                    table_name: dynamodb_ctx.dynamodb_table_name.clone(),
                                    item: inner_edge,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        })
                    }
                }

                transaction_batcher.load_many(transactions).await?;
                Ok(ResolvedValue::new(serde_json::json!({
                    "id": serde_json::Value::String(autogenerated_id),
                })))
            }
            DynamoMutationResolver::DeleteNode { id } => {
                let id_to_be_deleted = match id
                    .param(ctx, last_resolver_value.map(|x| x.data_resolved.borrow()))?
                    .expect("can't fail")
                {
                    Value::String(inner) => inner,
                    _ => {
                        return Err(Error::new("Internal Error: failed to infer key"));
                    }
                };

                let id_to_be_deleted_attr = id_to_be_deleted.clone().into_attr();
                let mut item = HashMap::new();

                item.insert("__pk".to_string(), id_to_be_deleted_attr.clone());
                item.insert("__sk".to_string(), id_to_be_deleted_attr.clone());

                let t = TxItem {
                    pk: id_to_be_deleted.clone(),
                    sk: id_to_be_deleted.clone(),
                    transaction: TransactWriteItem {
                        delete: Some(Delete {
                            expression_attribute_names: Some({
                                HashMap::from([("#pk".to_string(), "__pk".to_string())])
                            }),
                            condition_expression: Some("attribute_exists(#pk)".to_string()),
                            table_name: dynamodb_ctx.dynamodb_table_name.clone(),
                            key: item,
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                };

                transaction_batcher.load_one(t).await?;

                Ok(ResolvedValue::new(serde_json::json!({
                    "id": serde_json::Value::String(id_to_be_deleted),
                })))
            }
        }
    }
}
