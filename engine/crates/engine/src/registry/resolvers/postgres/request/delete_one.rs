use grafbase_sql_ast::renderer::{self, Renderer};
use postgres_types::transport::TransportExt;
use serde_json::Value;

use super::{log, query};
use crate::{
    registry::resolvers::{postgres::context::PostgresContext, ResolvedValue},
    Error,
};

pub(crate) async fn execute(ctx: PostgresContext<'_>) -> Result<ResolvedValue, Error> {
    let (sql, params) = renderer::Postgres::build(query::delete::build(&ctx, ctx.by_filter()?)?);

    if ctx.mutation_is_returning() {
        let operation = ctx.transport().collect_query(&sql, params);
        let rows = log::query(&ctx, &sql, operation).await?;
        let row = rows.into_iter().next().map(|row| row.root).unwrap_or(Value::Null);

        let row_count = if row.is_null() { 0 } else { 1 };

        Ok(ResolvedValue::new(serde_json::json!({
            "returning": row,
            "rowCount": row_count,
        })))
    } else {
        let operation = ctx.transport().parameterized_execute(&sql, params);
        let row_count = log::execute(&ctx, &sql, operation).await?;

        Ok(ResolvedValue::new(serde_json::json!({
            "rowCount": row_count,
        })))
    }
}
