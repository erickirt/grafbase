use wasmtime::component::{ComponentType, Lower};

use crate::state::InstanceState;

impl Host for InstanceState {}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct Schema<'a> {
    #[component(name = "type-definitions")]
    pub type_definitions: Vec<TypeDefinition<'a>>,
    pub directives: Vec<Directive<'a>>,
    #[component(name = "root-types")]
    pub root_types: RootTypes,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct RootTypes {
    #[component(name = "query-id")]
    pub query_id: Option<DefinitionId>,
    #[component(name = "mutation-id")]
    pub mutation_id: Option<DefinitionId>,
    #[component(name = "subscription-id")]
    pub subscription_id: Option<DefinitionId>,
}

pub type DefinitionId = u32;

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(variant)]
pub enum TypeDefinition<'a> {
    #[component(name = "scalar")]
    Scalar(ScalarDefinition<'a>),
    #[component(name = "object")]
    Object(ObjectDefinition<'a>),
    #[component(name = "interface")]
    Interface(InterfaceDefinition<'a>),
    #[component(name = "union")]
    Union(UnionDefinition<'a>),
    #[component(name = "enum")]
    Enum(EnumDefinition<'a>),
    #[component(name = "input-object")]
    InputObject(InputObjectDefinition<'a>),
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct ScalarDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    #[component(name = "specified-by-url")]
    pub specified_by_url: Option<&'a str>,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct ObjectDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    pub interfaces: Vec<DefinitionId>,
    pub fields: Vec<FieldDefinition<'a>>,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct InterfaceDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    pub interfaces: Vec<DefinitionId>,
    pub fields: Vec<FieldDefinition<'a>>,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct UnionDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    #[component(name = "member-types")]
    pub member_types: Vec<DefinitionId>,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct EnumDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    pub values: Vec<EnumValue<'a>>,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct InputObjectDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    #[component(name = "input-fields")]
    pub input_fields: Vec<InputValueDefinition<'a>>,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct FieldDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    pub ty: Ty,
    pub arguments: Vec<InputValueDefinition<'a>>,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct Ty {
    pub wrapping: Vec<WrappingType>,
    #[component(name = "definition-id")]
    pub definition_id: DefinitionId,
}

impl From<engine_schema::Type<'_>> for Ty {
    fn from(ty: engine_schema::Type<'_>) -> Self {
        Ty {
            wrapping: ty.wrapping.iter().map(Into::into).collect(),
            definition_id: ty.definition_id.as_guid(),
        }
    }
}

#[derive(Clone, Copy, Debug, ComponentType, Lower)]
#[component(enum)]
#[repr(u8)]
pub enum WrappingType {
    #[component(name = "non-null")]
    NonNull,
    #[component(name = "list")]
    List,
}

impl From<engine_schema::WrappingType> for WrappingType {
    fn from(wrapping: engine_schema::WrappingType) -> Self {
        match wrapping {
            engine_schema::WrappingType::NonNull => WrappingType::NonNull,
            engine_schema::WrappingType::List => WrappingType::List,
        }
    }
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct InputValueDefinition<'a> {
    pub id: DefinitionId,
    pub name: &'a str,
    pub ty: Ty,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct EnumValue<'a> {
    pub name: &'a str,
    pub directives: Vec<Directive<'a>>,
}

#[derive(Clone, Debug, ComponentType, Lower)]
#[component(record)]
pub struct Directive<'a> {
    pub name: &'a str,
    pub arguments: Vec<u8>,
}

// Typical Wasmtime bindgen! macro generated stuff
pub trait Host: Send + ::core::marker::Send {}
impl<_T: Host + ?Sized + Send> Host for &mut _T {}
pub fn add_to_linker<T, D>(
    _linker: &mut wasmtime::component::Linker<T>,
    _host_getter: fn(&mut T) -> D::Data<'_>,
) -> wasmtime::Result<()>
where
    D: wasmtime::component::HasData,
    for<'a> D::Data<'a>: Host,
    T: 'static + Send,
{
    Ok(())
}
