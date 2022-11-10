use dynaql_value::{ConstValue, Value};

use std::cmp::Ordering;

use crate::registry::MetaInputValue;
use crate::validation::visitor::VisitorContext;

use super::DynValidate;
use crate::Pos;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LengthValidator {
    min: Option<usize>,
    max: Option<usize>,
}

impl LengthValidator {
    pub fn new(min: Option<usize>, max: Option<usize>) -> Self {
        LengthValidator { min, max }
    }
}

enum LengthTestResult {
    TooShort,
    TooLong,
    InBounds,
}

fn check_bounds<T: PartialOrd>(item: T, lower: Option<T>, upper: Option<T>) -> LengthTestResult {
    match (
        lower.as_ref().and_then(|lower| item.partial_cmp(lower)),
        upper.as_ref().and_then(|upper| item.partial_cmp(upper)),
    ) {
        (Some(Ordering::Less), _) => LengthTestResult::TooShort,
        (_, Some(Ordering::Greater)) => LengthTestResult::TooLong,
        (
            None | Some(Ordering::Greater | Ordering::Equal),
            None | Some(Ordering::Less | Ordering::Equal),
        ) => LengthTestResult::InBounds,
    }
}

impl DynValidate<&Value> for LengthValidator {
    fn validate<'a, 'b>(
        &self,
        ctx: &mut VisitorContext<'a>,
        meta: &'b MetaInputValue,
        pos: Pos,
        value: &Value,
    ) {
        use LengthTestResult::*;

        let var_value = match value {
            Value::Variable(var_name) => ctx
                .variables
                .and_then(|variables| variables.get(var_name).cloned().map(ConstValue::into_value)),
            _ => None,
        };
        let count = match var_value.as_ref().unwrap_or(value) {
            Value::List(values) => values.len(),
            Value::String(string) => string.chars().count(),
            _ => return,
        };
        let name = meta.name.as_str();
        match check_bounds(count, self.min, self.max) {
            InBounds => (),
            TooLong => ctx.report_error(
                vec![pos],
                format!(
                    "Invalid value for argument \"{name}\", length {count} is too long, must be less than {}",
                    self.max
                        .expect("max must have been some for this case to be hit")
                ),
            ),
            TooShort => ctx.report_error(
                vec![pos],
                format!(
                    "Invalid value for argument \"{name}\", length {count} is too short, must be more than {}",
                    self.min
                        .expect("min must have been some for this case to be hit")
                ),
            ),
        }
    }
}

#[test]
fn test_length_validator() {
    use super::{DynValidator, MetaInputValue};
    use crate::parser::parse_query;
    use crate::registry::MetaTypeName;
    use crate::validation::{visitor::test::visit_input_value, VisitorNil};
    use crate::{EmptyMutation, EmptySubscription, Object, Schema};
    use insta::assert_snapshot;

    struct Query;

    #[Object(internal)]
    #[allow(unreachable_code)]
    impl Query {
        async fn value(&self) -> i32 {
            todo!()
        }
    }

    let registry = Schema::create_registry_static::<Query, EmptyMutation, EmptySubscription>();
    let query = r#"{
        value #1
    }"#;

    let doc = parse_query(query).unwrap();

    let meta = MetaInputValue {
        name: "test".to_string(),
        description: None,
        ty: "String".to_string(),
        default_value: None,
        validators: None,
        visible: None,
        is_secret: false,
    };

    let mut ctx = VisitorContext::new(&registry, &doc, None);
    let custom_validator = DynValidator::length(Some(0), None);
    custom_validator.validate(
        &mut ctx,
        &meta,
        Pos::from((0, 0)),
        &Value::String("test".to_string()),
    );
    assert!(ctx.errors.is_empty());

    let mut ctx = VisitorContext::new(&registry, &doc, None);
    let custom_validator = DynValidator::length(Some(0), Some(1));
    custom_validator.validate(
        &mut ctx,
        &meta,
        Pos::from((0, 0)),
        &Value::String("test".to_string()),
    );
    assert_eq!(ctx.errors.len(), 1);
    assert_snapshot!(ctx.errors[0].message);

    let mut ctx = VisitorContext::new(&registry, &doc, None);
    let custom_validator = DynValidator::length(Some(10), Some(15));
    custom_validator.validate(
        &mut ctx,
        &meta,
        Pos::from((0, 0)),
        &Value::String("test".to_string()),
    );
    assert_eq!(ctx.errors.len(), 1, "{:#?}", ctx.errors);
    assert_snapshot!(ctx.errors[0].message);

    let vars = crate::Variables::from_json(serde_json::json!({"test":"test"}));
    let mut ctx = VisitorContext::new(&registry, &doc, Some(&vars));
    let custom_validator = DynValidator::length(Some(10), Some(15));
    custom_validator.validate(
        &mut ctx,
        &meta,
        Pos::from((0, 0)),
        &Value::Variable(dynaql_value::Name::new("test")),
    );
    assert_eq!(ctx.errors.len(), 1, "{:#?}", ctx.errors);
    assert_snapshot!(ctx.errors[0].message);

    // Test nested validation via the visitor
    let custom_validator = DynValidator::length(Some(10), Some(15));
    let meta = MetaInputValue {
        name: "test".to_string(),
        description: None,
        ty: "[String]".to_string(),
        default_value: None,
        validators: Some(vec![custom_validator]),
        visible: None,
        is_secret: false,
    };
    let mut visitor = VisitorNil;
    let mut ctx = VisitorContext::new(&registry, &doc, None);
    let value = Value::List(vec![Value::String("test".to_string())]);
    visit_input_value(
        &mut visitor,
        &mut ctx,
        Pos::from((0, 0)),
        Some(MetaTypeName::List("String")),
        &value,
        Some(&meta),
    );
    assert_eq!(ctx.errors.len(), 1, "{:#?}", ctx.errors);
    assert_snapshot!(ctx.errors[0].message);
}
