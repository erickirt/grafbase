use std::sync::Arc;

use engine::EngineOperationContext;
use engine_error::{ErrorCode, ErrorResponse, GraphqlError};
use engine_schema::{GraphqlSubgraph, VirtualSubgraph};
use event_queue::EventQueue;
use futures::future::BoxFuture;
use http::{request, response};
use runtime::extension::{ExtensionRequestContext, OnRequest, ReqwestParts};
use url::Url;

use crate::{
    extension::{
        HooksExtensionInstance,
        api::since_0_21_0::wit::{self, HttpMethod, HttpRequestPartsParam},
    },
    resources::EventQueueResource,
};

impl HooksExtensionInstance for super::ExtensionInstanceSince0_21_0 {
    fn on_request<'a>(
        &'a mut self,
        event_queue: EventQueue,
        mut parts: request::Parts,
    ) -> BoxFuture<'a, wasmtime::Result<Result<OnRequest, ErrorResponse>>> {
        Box::pin(async move {
            let method: HttpMethod = (&parts.method).try_into()?;
            let url = parts.uri.to_string();
            let headers = std::mem::take(&mut parts.headers);
            let event_queue = Arc::new(event_queue);

            let resources = &mut self.store.data_mut().resources;
            let headers = resources.push(wit::Headers::from(headers))?;
            let ctx = resources.push(wit::HostContext::from(&event_queue))?;

            let result = self
                .inner
                .grafbase_sdk_hooks()
                .call_on_request(
                    &mut self.store,
                    ctx,
                    HttpRequestPartsParam {
                        url: url.as_str(),
                        method,
                        headers,
                    },
                )
                .await?;

            let output = match result {
                Ok(wit::OnRequestOutput {
                    headers,
                    contract_key,
                    context,
                }) => {
                    parts.headers = self.store.data_mut().resources.delete(headers)?.into_inner().unwrap();
                    Ok(OnRequest {
                        parts,
                        contract_key,
                        context: ExtensionRequestContext {
                            event_queue,
                            hooks_context: Default::default(),
                        },
                    })
                }
                Err(err) => Err(self
                    .store
                    .data_mut()
                    .take_error_response(err, ErrorCode::ExtensionError)?),
            };

            Ok(output)
        })
    }

    fn on_response(
        &mut self,
        context: ExtensionRequestContext,
        mut parts: response::Parts,
    ) -> BoxFuture<'_, wasmtime::Result<Result<response::Parts, String>>> {
        Box::pin(async move {
            let headers = std::mem::take(&mut parts.headers);
            let status = parts.status.as_u16();

            let resources = &mut self.store.data_mut().resources;
            let headers = resources.push(wit::Headers::from(headers))?;
            let queue = resources.push(EventQueueResource(context.event_queue.clone()))?;
            let host_context = resources.push(wit::HostContext::from(&context))?;
            let ctx = resources.push(wit::RequestContext::from(&context))?;

            let result = self
                .inner
                .grafbase_sdk_hooks()
                .call_on_response(&mut self.store, host_context, ctx, status, headers, queue)
                .await?;

            let result = match result {
                Ok(headers) => {
                    parts.headers = self.store.data_mut().resources.delete(headers)?.into_inner().unwrap();
                    Ok(parts)
                }
                Err(err) => Err(err),
            };
            Ok(result)
        })
    }

    fn on_graphql_subgraph_request<'a>(
        &'a mut self,
        ctx: EngineOperationContext,
        subgraph: GraphqlSubgraph<'a>,
        ReqwestParts { url, method, headers }: ReqwestParts,
    ) -> BoxFuture<'a, wasmtime::Result<Result<ReqwestParts, GraphqlError>>> {
        Box::pin(async move {
            let method: HttpMethod = (&method).try_into()?;

            let resources = &mut self.store.data_mut().resources;
            let headers = resources.push(wit::Headers::from(headers))?;
            let host_context = resources.push(wit::HostContext::from(&ctx))?;
            let ctx = resources.push(ctx)?;

            let result = self
                .inner
                .grafbase_sdk_hooks()
                .call_on_graphql_subgraph_request(
                    &mut self.store,
                    host_context,
                    ctx,
                    subgraph.name(),
                    HttpRequestPartsParam {
                        url: url.as_str(),
                        method,
                        headers,
                    },
                )
                .await?;

            let result = match result {
                Ok(parts) => {
                    let headers = self
                        .store
                        .data_mut()
                        .resources
                        .delete(parts.headers)?
                        .into_inner()
                        .unwrap();
                    // Must be *after* the headers, to ensure the wasm store is kept clean.
                    let url = match parts.url.parse::<Url>() {
                        Ok(url) => url,
                        Err(err) => {
                            tracing::error!("Invalid URL ({:?}) returned by extension: {err}", parts.url);
                            return Ok(Err(GraphqlError::internal_extension_error()));
                        }
                    };

                    Ok(ReqwestParts {
                        url,
                        method: parts.method.into(),
                        headers,
                    })
                }
                Err(err) => Err(err.into_graphql_error(ErrorCode::ExtensionError)),
            };
            Ok(result)
        })
    }

    fn on_virtual_subgraph_request<'a>(
        &'a mut self,
        ctx: EngineOperationContext,
        subgraph: VirtualSubgraph<'a>,
        headers: http::HeaderMap,
    ) -> BoxFuture<'a, wasmtime::Result<Result<http::HeaderMap, GraphqlError>>> {
        Box::pin(async move {
            let resources = &mut self.store.data_mut().resources;
            let headers = resources.push(wit::Headers::from(headers))?;
            let host_context = resources.push(wit::HostContext::from(&ctx))?;
            let ctx = resources.push(ctx)?;

            let result = self
                .inner
                .grafbase_sdk_hooks()
                .call_on_virtual_subgraph_request(&mut self.store, host_context, ctx, subgraph.name(), headers)
                .await?;

            let result = match result {
                Ok(headers) => {
                    let headers = self.store.data_mut().resources.delete(headers)?.into_inner().unwrap();
                    Ok(headers)
                }
                Err(err) => Err(err.into_graphql_error(ErrorCode::ExtensionError)),
            };
            Ok(result)
        })
    }
}
