use std::cmp::Ordering;

use crate::{ExtensionDirective, Schema, StringId};

use super::ExtensionInputValueRecord;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, serde::Serialize, serde::Deserialize, id_derives::Id)]
pub struct ExtensionDirectiveArgumentId(u32);

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExtensionDirectiveArgumentRecord {
    pub name_id: StringId,
    pub value: ExtensionInputValueRecord,
    pub injection_stage: InjectionStage,
}

impl<'a> ExtensionDirective<'a> {
    pub fn argument_records(&self) -> &'a [ExtensionDirectiveArgumentRecord] {
        &self.schema[self.as_ref().argument_ids]
    }

    pub fn static_arguments(&self) -> ExtensionDirectiveArgumentsStaticView<'a> {
        ExtensionDirectiveArgumentsStaticView {
            schema: self.schema,
            ref_: self.argument_records(),
        }
    }
}

// When, at the earliest, can we compute the argument's value?
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InjectionStage {
    // No data injection, static data
    #[default]
    Static,
    // Injects data from the field arguments
    Query,
    // Injects data from the response such as fields
    Response,
}

impl PartialOrd for InjectionStage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InjectionStage {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Response, Self::Response) => Ordering::Equal,
            (Self::Response, Self::Query) | (Self::Response, Self::Static) => Ordering::Greater,
            (Self::Query, Self::Response) => Ordering::Less,
            (Self::Query, Self::Query) => Ordering::Equal,
            (Self::Query, Self::Static) => Ordering::Greater,
            (Self::Static, Self::Query) | (Self::Static, Self::Response) => Ordering::Less,
            (Self::Static, Self::Static) => Ordering::Equal,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ExtensionDirectiveArgumentsStaticView<'a> {
    schema: &'a Schema,
    ref_: &'a [ExtensionDirectiveArgumentRecord],
}

impl serde::Serialize for ExtensionDirectiveArgumentsStaticView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Self { schema, ref_ } = *self;
        serializer.collect_map(
            ref_.iter()
                .filter(|arg| matches!(arg.injection_stage, InjectionStage::Static))
                .map(|arg| {
                    (
                        &schema[arg.name_id],
                        ExtensionInputValueStaticView {
                            schema,
                            ref_: &arg.value,
                        },
                    )
                }),
        )
    }
}

#[derive(Clone, Copy)]
struct ExtensionInputValueStaticView<'a> {
    schema: &'a Schema,
    ref_: &'a ExtensionInputValueRecord,
}

impl std::fmt::Debug for ExtensionInputValueStaticView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionInputValueStaticView").finish_non_exhaustive()
    }
}

impl serde::Serialize for ExtensionInputValueStaticView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Self { schema, ref_ } = *self;
        match ref_ {
            ExtensionInputValueRecord::Null => serializer.serialize_none(),
            ExtensionInputValueRecord::String(id) => serializer.serialize_str(&schema[*id]),
            ExtensionInputValueRecord::EnumValue(id) => serializer.serialize_str(&schema[*id]),
            ExtensionInputValueRecord::Int(value) => serializer.serialize_i32(*value),
            ExtensionInputValueRecord::BigInt(value) => serializer.serialize_i64(*value),
            ExtensionInputValueRecord::U64(value) => serializer.serialize_u64(*value),
            ExtensionInputValueRecord::Float(value) => serializer.serialize_f64(*value),
            ExtensionInputValueRecord::Boolean(value) => serializer.serialize_bool(*value),
            ExtensionInputValueRecord::Map(map) => {
                serializer.collect_map(map.iter().map(|(key, ref_)| (&schema[*key], Self { schema, ref_ })))
            }
            ExtensionInputValueRecord::List(list) => {
                serializer.collect_seq(list.iter().map(|ref_| Self { schema, ref_ }))
            }
            ExtensionInputValueRecord::FieldSet(_)
            | ExtensionInputValueRecord::InputValueSet(_)
            | ExtensionInputValueRecord::Template(_) => {
                unreachable!("Invariant broken, cannot be a static value.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injection_stage_ordering() {
        assert!(InjectionStage::Static < InjectionStage::Query);
        assert!(InjectionStage::Query < InjectionStage::Response);
        assert!(InjectionStage::Static < InjectionStage::Response);
    }
}
