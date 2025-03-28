use engine_error::{ErrorCode, GraphqlError};

use crate::{cbor, extension::api::since_0_8_0::wit::grafbase::sdk::types};

impl types::Error {
    pub(crate) fn into_graphql_error(self, code: ErrorCode) -> GraphqlError {
        GraphqlError::new(self.message, code).with_extensions(self.extensions.into_iter().map(|(key, value)| {
            let value: serde_json::Value = cbor::from_slice(&value).unwrap_or_default();
            (key, value)
        }))
    }
}
