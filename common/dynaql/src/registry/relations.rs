use dynaql_parser::types::{BaseType, Type};

#[derive(Clone, PartialEq, Eq, Debug, serde::Deserialize, serde::Serialize, Hash)]
pub enum MetaRelationKind {
    ManyToMany,
    ManyToOne,
    OneToMany,
    OneToOne,
}

// TODO: Add a link to the documentation related to this.
#[derive(Debug, thiserror::Error)]
pub enum RelationCombinationError {
    #[error(
        "You have an issue while modelizeing your relations. Try using the `@relation` directive."
    )]
    UndefinedError,
    #[error("You have multiple relations starting from `{from}`, the relation engine can't define them without a little help. Try using the `@relation` directive.")]
    MultipleRelationsError { from: String },
    #[error("You have an impossible combination ongoing.")]
    ImpossibleCombination,
}

impl MetaRelationKind {
    #[allow(dead_code)]
    const fn inverse(&self) -> Self {
        match &self {
            Self::ManyToMany => Self::ManyToMany,
            Self::OneToMany => Self::ManyToOne,
            Self::ManyToOne => Self::OneToMany,
            Self::OneToOne => Self::OneToOne,
        }
    }

    pub fn new(from: &Type, to: &Type) -> Self {
        match (&from.base, &to.base) {
            (BaseType::Named(_), BaseType::Named(_)) => Self::OneToOne,
            (BaseType::Named(_), BaseType::List(_)) => Self::OneToMany,
            (BaseType::List(_), BaseType::Named(_)) => Self::ManyToOne,
            (BaseType::List(_), BaseType::List(_)) => Self::ManyToMany,
        }
    }
}

/// Define relations across types.
///
/// # Mono-directional relation
///
/// In case of mono-directional relation, for instance:
///
/// Node A -> Node B
///
/// The Relation associated to that will always be a ManyToOne, `A* -> 1 B`.
/// The inner `Type` will represent the next edge of the relation.
///
/// NB: In case of Mono-directional relation, adding the other edge of the relation
/// in the GraphQL schema won't add the relation from the other direction, why?
/// Because if it's a mono-directional relation, we do not store the other edge
/// of the relation.
///
/// # Example
///
/// ## OneToOne
///
/// ### Author OneToOne Post ->
///
/// ```graphql
/// type Author @modelized {
///   published: Post
/// }
///
/// type Post @modelized {
///   ... (Not connected)
/// }
/// ```
///
/// ### Author OneToOne Post <->
///
/// ```graphql
/// type Author @modelized {
///   published: Post
/// }
///
/// type Post @modelized {
///   author: Author
/// }
/// ```
///
/// ## OneToMany
///
/// ### Author OneToMany Post ->
///
/// ```graphql
/// type Author @modelized {
///   published: [Post]
/// }
///
/// type Post @modelized {
///   ... (Not connected)
/// }
/// ```
///
/// ### Author OneToMany Post <->
///
/// ```graphql
/// type Author @modelized {
///   published: [Post]
/// }
///
/// type Post @modelized {
///   author: Author
/// }
/// ```
///
/// ## ManyToOne
///
/// ### Author ManyToOne Post ->
///
/// ```graphql
/// type Author @modelized {
///   published: Post
/// }
///
/// type Post @modelized {
///   ... (Not connected)
/// }
/// ```
///
/// ### Author ManyToOne Post <->
///
/// ```graphql
/// type Author @modelized {
///   published: Post
/// }
///
/// type Post @modelized {
///   authors: [Author]
/// }
/// ```
///
/// ## ManyToMany
///
/// ### Author ManyToMany Post ->
///
/// ```graphql
/// type Author @modelized {
///   published: [Post]
/// }
///
/// type Post @modelized {
///   ... (Not connected)
/// }
/// ```
///
/// ### Author ManyToMany Post <->
///
/// ```graphql
/// type Author @modelized {
///   published: [Post]
/// }
///
/// type Post @modelized {
///   authors: [Author]
/// }
/// ```
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, Hash, PartialEq, Eq)]
pub struct MetaRelation {
    pub name: String,
    pub kind: MetaRelationKind,
    /// The direction is:
    /// 0 -> 1
    /// The relation can have a null origin, it means it's everything related to 1.
    pub relation: (Option<String>, String),
    pub birectional: bool,
}

impl MetaRelation {
    fn generate_relation_name(a: &str, b: &str) -> String {
        let mut a = vec![a, b];
        a.sort_unstable();
        a.join("Relate")
    }

    pub fn new(name: Option<String>, from: &Type, to: &Type) -> Self {
        let base_from = from.base.to_base_type_str();
        let base_to = to.base.to_base_type_str();
        Self {
            name: name
                .unwrap_or_else(|| MetaRelation::generate_relation_name(&base_from, &base_to)),
            relation: (Some(base_from.to_string()), base_to.to_string()),
            birectional: false,
            kind: MetaRelationKind::new(&from, &to),
        }
    }

    /// Create a new collection relation
    pub fn base_collection_relation(name: String, to: &Type) -> Self {
        let base_to = to.base.to_base_type_str();
        Self {
            name,
            relation: (None, base_to.to_string()),
            birectional: false,
            kind: MetaRelationKind::OneToMany,
        }
    }

    /// Compose the relation with another relation based on the realtion's name
    pub fn with(&mut self, relation: MetaRelation) -> Result<(), RelationCombinationError> {
        // The relation start with a OneToX, it's obligated.
        if self.name != relation.name {
            return Err(RelationCombinationError::UndefinedError);
        }

        if self
            .relation
            .0
            .as_ref()
            .ok_or(RelationCombinationError::ImpossibleCombination)?
            .ne(&relation.relation.1)
            || self.relation.1.ne(relation
                .relation
                .0
                .as_ref()
                .ok_or(RelationCombinationError::ImpossibleCombination)?)
        {
            return Err(RelationCombinationError::MultipleRelationsError {
                from: self
                    .relation
                    .0
                    .clone()
                    .ok_or(RelationCombinationError::ImpossibleCombination)?,
            });
        }

        // TODO: Need to redefine this
        self.kind = match (&self.kind, relation.kind) {
            (MetaRelationKind::OneToOne, MetaRelationKind::OneToOne) => MetaRelationKind::OneToOne,
            (MetaRelationKind::OneToOne, MetaRelationKind::OneToMany) => {
                MetaRelationKind::ManyToOne
            }
            (MetaRelationKind::OneToMany, MetaRelationKind::OneToOne) => {
                MetaRelationKind::OneToMany
            }
            (MetaRelationKind::OneToMany, MetaRelationKind::OneToMany) => {
                MetaRelationKind::ManyToMany
            }
            _ => {
                return Err(RelationCombinationError::UndefinedError);
            }
        };

        self.birectional = true;
        Ok(())
    }
}
