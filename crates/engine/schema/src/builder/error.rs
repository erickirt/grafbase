use crate::StringId;

use super::{coerce::InputValueError, BuildContext};

#[derive(Debug, Copy, Clone)]
pub enum SchemaLocation {
    Definition { name: StringId },
    Field { ty: StringId, name: StringId },
}

impl SchemaLocation {
    pub fn to_string(self, ctx: &BuildContext<'_>) -> String {
        match self {
            SchemaLocation::Definition { name } => ctx.strings[name].to_string(),
            SchemaLocation::Field { ty, name } => format!("{}.{}", &ctx.strings[ty], &ctx.strings[name]),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("At {location}, a required field argument is invalid: {err}")]
    RequiredFieldArgumentCoercionError { location: String, err: InputValueError },
    #[error("An input value named '{name}' has an invalid default value: {err}")]
    DefaultValueCoercionError { name: String, err: InputValueError },
    #[error(transparent)]
    GraphFromSdlError(#[from] federated_graph::DomainError),
    #[error("Unsupported extension: {id}")]
    UnsupportedExtension { id: Box<extension_catalog::Id> },
    #[error("Could not load extension at '{url}': {err}")]
    CouldNotLoadExtension { url: String, err: String },
}
