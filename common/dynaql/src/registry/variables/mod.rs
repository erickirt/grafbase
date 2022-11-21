#![allow(deprecated)]

//! Variable Resolving definition strategy is explained here.
//!
//! When you need a Variable inside a Resolver, you can use a
//! `VariableResolveDefinition` struct to define how the graphql server should
//! resolve this variable.
use crate::{context::resolver_data_get_opt_ref, Context, Value};
use crate::{Error, ServerError};
use dynaql_value::Name;
use graph_entities::cursor::PaginationCursor;
use indexmap::IndexMap;

pub mod id;

/// Describe what should be done by the GraphQL Server to resolve this Variable.
#[non_exhaustive]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, Hash, PartialEq, Eq)]
pub enum VariableResolveDefinition {
    /// A Debug VariableResolveDefinition where you can just put the Value you
    /// would like to have.
    DebugString(String),
    /// Check the last Resolver in the Query Graph and try to resolve the
    /// variable defined in this field.
    InputTypeName(String),
    /// Resolve a Value by querying the ResolverContextData with a key_id.
    /// What is store in the ResolverContextData is described on each Resolver
    /// implementation.
    ///
    #[deprecated = "Should not use Context anymore in SDL def"]
    ResolverData(String),
    /// Resolve a Value by querying the most recent ancestor resolver property.
    LocalData(String),
}

impl VariableResolveDefinition {
    /// Resolve the first variable with this definition
    pub fn param<'a>(
        &self,
        ctx: &'a Context<'a>,
        last_resolver_value: Option<&'a serde_json::Value>,
    ) -> Result<Option<Value>, ServerError> {
        match self {
            Self::InputTypeName(name) => Ok(ctx
                .resolver_node
                .as_ref()
                .and_then(|resolver| resolver.get_variable_by_name(name))
                .map(|x| x.transform_to_variables_resolved(ctx))
                .transpose()?
                .map(|(_, x)| x)),
            #[allow(deprecated)]
            Self::ResolverData(key) => Ok(resolver_data_get_opt_ref::<Value>(
                &ctx.resolvers_data.read().expect("handle"),
                key,
            )
            .map(std::clone::Clone::clone)),
            Self::DebugString(inner) => Ok(Some(Value::String(inner.clone()))),
            Self::LocalData(inner) => {
                let result = last_resolver_value
                    .and_then(|x| x.get(inner))
                    .map(std::borrow::ToOwned::to_owned)
                    .unwrap_or_else(|| serde_json::Value::Null);

                Ok(Value::from_json(result).ok())
            }
        }
    }

    pub fn expect_string<'a>(
        &self,
        ctx: &'a Context<'a>,
        last_resolver_value: Option<&'a serde_json::Value>,
    ) -> Result<String, ServerError> {
        match self.param(ctx, last_resolver_value)? {
            Some(Value::String(inner)) => Ok(inner),
            _ => {
                Err(Error::new("Internal Error: failed to infer key")
                    .into_server_error(ctx.item.pos))
            }
        }
    }

    pub fn expect_obj<'a>(
        &self,
        ctx: &'a Context<'a>,
        last_resolver_value: Option<&'a serde_json::Value>,
    ) -> Result<IndexMap<Name, Value>, ServerError> {
        match self.param(ctx, last_resolver_value)? {
            Some(Value::Object(inner)) => Ok(inner),
            _ => {
                Err(Error::new("Internal Error: failed to infer key")
                    .into_server_error(ctx.item.pos))
            }
        }
    }

    pub fn expect_opt_string<'a>(
        &self,
        ctx: &'a Context<'a>,
        last_resolver_value: Option<&'a serde_json::Value>,
    ) -> Result<Option<String>, ServerError> {
        match self.param(ctx, last_resolver_value)? {
            Some(Value::String(inner)) => Ok(Some(inner)),
            Some(Value::Null) => Ok(None),
            None => Ok(None),
            _ => {
                Err(Error::new("Internal Error: failed to infer key")
                    .into_server_error(ctx.item.pos))
            }
        }
    }

    pub fn expect_opt_cursor<'a>(
        &self,
        ctx: &'a Context<'a>,
        last_resolver_value: Option<&'a serde_json::Value>,
    ) -> Result<Option<PaginationCursor>, ServerError> {
        match self.expect_opt_string(ctx, last_resolver_value)? {
            Some(s) => match PaginationCursor::from_string(s) {
                Ok(cursor) => Ok(Some(cursor)),
                Err(_) => Err(Error::new("Invalid Cursor").into_server_error(ctx.item.pos)),
            },
            None => Ok(None),
        }
    }

    pub fn expect_opt_int<'a>(
        &self,
        ctx: &'a Context<'a>,
        last_resolver_value: Option<&'a serde_json::Value>,
        limit: Option<usize>,
    ) -> Result<Option<usize>, ServerError> {
        let result = match self.param(ctx, last_resolver_value)? {
            Some(Value::Number(inner)) => inner
                .as_u64()
                .ok_or_else(|| Error::new("Internal Error: failed to infer Int"))
                .map(|value| value as usize)
                .and_then(|value| match limit {
                    Some(limit) if value > limit => Err(Error::new(format!(
                        "Limit Error: the integer must be smaller than {}",
                        limit
                    ))),
                    _ => Ok(Some(value as usize)),
                }),
            Some(Value::Null) | None => Ok(None),
            _ => Err(Error::new("Internal Error: failed to infer key")),
        };
        result.map_err(|err| err.into_server_error(ctx.item.pos))
    }
}
