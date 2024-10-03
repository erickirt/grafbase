use super::{EnumId, InputObjectId, InterfaceId, ObjectId, TypeDefinitionId, UnionId, Wrapping};

#[derive(Clone, PartialEq, Eq)]
pub struct Type {
    pub wrapping: Wrapping,
    pub definition: Definition,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Definition {
    Scalar(TypeDefinitionId),
    Object(ObjectId),
    Interface(InterfaceId),
    Union(UnionId),
    Enum(EnumId),
    InputObject(InputObjectId),
}

impl Definition {
    pub fn as_enum(&self) -> Option<EnumId> {
        if let Self::Enum(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}
