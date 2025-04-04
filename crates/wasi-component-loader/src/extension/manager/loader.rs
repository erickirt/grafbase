use std::sync::Arc;

use super::{ExtensionConfig, ExtensionInstance, WasmConfig};
use crate::{
    cache::Cache,
    config::build_extensions_context,
    extension::api::{SdkPre, wit},
    resources::SharedResources,
    state::WasiState,
};
use anyhow::Context;
use wasmtime::{
    Engine,
    component::{Component, Linker},
};

pub struct ExtensionLoader {
    wasm_config: WasmConfig,
    guest_config: Vec<u8>,
    #[allow(unused)] // MUST be unused, or at least immutable, we self-reference to it
    schema_directives: Vec<SchemaDirective>,
    // Self-reference to schema_directives
    wit_schema_directives: Vec<wit::SchemaDirective<'static>>,
    pre: SdkPre,
    cache: Arc<Cache>,
    shared: SharedResources,
}

pub struct SchemaDirective {
    name: String,
    subgraph_name: String,
    arguments: Vec<u8>,
}

impl SchemaDirective {
    pub fn new<T: serde::Serialize>(name: impl Into<String>, subgraph_name: impl Into<String>, args: T) -> Self {
        Self {
            name: name.into(),
            subgraph_name: subgraph_name.into(),
            arguments: crate::cbor::to_vec(args).unwrap(),
        }
    }
}

impl ExtensionLoader {
    pub fn new<T: serde::Serialize>(shared: SharedResources, config: ExtensionConfig<T>) -> crate::Result<Self> {
        let mut engine_config = wasmtime::Config::new();

        engine_config.wasm_component_model(true);
        engine_config.async_support(true);

        let engine = Engine::new(&engine_config)?;
        let component = Component::from_file(&engine, &config.wasm.location)?;

        tracing::debug!(
            location = config.wasm.location.to_str(),
            "loaded the provided web assembly component successfully",
        );

        let mut linker = Linker::<WasiState>::new(&engine);

        // adds the wasi interfaces to our component
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        if config.wasm.networking {
            // adds the wasi http interfaces to our component
            wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;
        }

        let pre = SdkPre::initialize(&config.sdk_version, component, linker)?;

        let wit_schema_directives = config
            .schema_directives
            .iter()
            .map(|dir| {
                let dir = wit::SchemaDirective {
                    name: &dir.name,
                    subgraph_name: &dir.subgraph_name,
                    arguments: &dir.arguments,
                };
                // SAFETY: Self-reference to schema_directives which is kept alive and never
                // changed.
                let dir: wit::SchemaDirective<'static> = unsafe { std::mem::transmute(dir) };
                dir
            })
            .collect();

        Ok(Self {
            shared,
            wasm_config: config.wasm,
            guest_config: crate::cbor::to_vec(&config.guest_config).context("Could not serialize configuration")?,
            schema_directives: config.schema_directives,
            wit_schema_directives,
            pre,
            cache: Arc::new(Cache::new()),
        })
    }

    pub async fn instantiate(&self) -> crate::Result<Box<dyn ExtensionInstance>> {
        let state = WasiState::new(
            build_extensions_context(&self.wasm_config),
            self.shared.access_log.clone(),
            self.cache.clone(),
            self.wasm_config.networking,
        );

        self.pre
            .instantiate(state, &self.wit_schema_directives, &self.guest_config)
            .await
    }
}
