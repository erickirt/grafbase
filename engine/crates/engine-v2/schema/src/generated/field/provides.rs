//! ===================
//! !!! DO NOT EDIT !!!
//! ===================
//! Automatically generated by engine-v2-codegen from domain/schema.graphql
use crate::{
    generated::{Subgraph, SubgraphId},
    prelude::*,
    ProvidableFieldSet,
};
use readable::Readable;

/// Generated from:
///
/// ```custom,{.language-graphql}
/// type FieldProvides @meta(module: "field/provides") {
///   subgraph: Subgraph!
///   field_set: ProvidableFieldSet!
/// }
/// ```
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FieldProvidesRecord {
    pub subgraph_id: SubgraphId,
    pub field_set: ProvidableFieldSet,
}

#[derive(Clone, Copy)]
pub struct FieldProvides<'a> {
    pub(crate) schema: &'a Schema,
    pub(crate) ref_: &'a FieldProvidesRecord,
}

impl std::ops::Deref for FieldProvides<'_> {
    type Target = FieldProvidesRecord;
    fn deref(&self) -> &Self::Target {
        self.ref_
    }
}

impl<'a> FieldProvides<'a> {
    #[allow(clippy::should_implement_trait)]
    pub fn as_ref(&self) -> &'a FieldProvidesRecord {
        self.ref_
    }
    pub fn subgraph(&self) -> Subgraph<'a> {
        self.as_ref().subgraph_id.read(self.schema)
    }
}

impl Readable<Schema> for &FieldProvidesRecord {
    type Reader < 'a > = FieldProvides < 'a > where Self : 'a ;
    fn read<'s>(self, schema: &'s Schema) -> Self::Reader<'s>
    where
        Self: 's,
    {
        FieldProvides { schema, ref_: self }
    }
}

impl std::fmt::Debug for FieldProvides<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldProvides")
            .field("subgraph", &self.subgraph())
            .field("field_set", &self.field_set)
            .finish()
    }
}
