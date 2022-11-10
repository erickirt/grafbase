use dynaql_value::Value;

use crate::registry::MetaInputValue;
use crate::validation::visitor::VisitorContext;
use crate::Pos;

mod length;

use length::LengthValidator;

pub(crate) trait DynValidate<T> {
    fn validate<'a, 'b>(
        &self,
        _ctx: &mut VisitorContext<'a>,
        meta: &'b MetaInputValue,
        pos: Pos,
        other: T,
    );
}

// Wrap Validators up in an enum to avoid having to box the context data
#[derive(Clone, derivative::Derivative, serde::Serialize, serde::Deserialize)]
pub enum DynValidator {
    Length(LengthValidator),
}

impl DynValidator {
    pub fn length(min: Option<usize>, max: Option<usize>) -> Self {
        Self::Length(LengthValidator::new(min, max))
    }
}

impl DynValidator {
    fn inner(&self) -> &dyn DynValidate<&Value> {
        use DynValidator::*;
        #[allow(clippy::single_match)]
        match self {
            Length(v) => v,
        }
    }
}

impl DynValidate<&Value> for DynValidator {
    fn validate<'a, 'b>(
        &self,
        ctx: &mut VisitorContext<'a>,
        meta: &'b MetaInputValue,
        pos: Pos,
        value: &Value,
    ) {
        self.inner().validate(ctx, meta, pos, value)
    }
}
