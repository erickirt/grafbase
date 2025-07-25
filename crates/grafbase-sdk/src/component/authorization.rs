use crate::{types::Headers, wit};

use super::{Component, state};

impl wit::AuthorizationGuest for Component {
    fn authorize_query(
        context: wit::SharedContext,
        headers: wit::Headers,
        token: wit::Token,
        elements: wit::QueryElements,
    ) -> Result<wit::AuthorizationOutput, wit::ErrorResponse> {
        state::with_context(context, || {
            let mut headers: Headers = headers.into();
            state::extension()?
                .authorize_query(&mut headers, token.into(), (&elements).into())
                .map(|(decisions, state)| wit::AuthorizationOutput {
                    decisions: decisions.into(),
                    state,
                    headers: headers.into(),
                })
                .map_err(Into::into)
        })
    }

    fn authorize_response(
        context: wit::SharedContext,
        state: Vec<u8>,
        elements: wit::ResponseElements,
    ) -> Result<wit::AuthorizationDecisions, wit::Error> {
        state::with_context(context, || {
            state::extension()?
                .authorize_response(state, (&elements).into())
                .map(Into::into)
                .map_err(Into::into)
        })
    }
}
