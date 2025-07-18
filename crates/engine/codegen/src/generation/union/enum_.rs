use case::CaseExt;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{TokenStreamExt, quote};
use tracing::instrument;

use crate::{
    domain::{Domain, Union, UnionKind},
    generation::docstr,
};

use super::{VariantContext, debug::DebugVariantBranch};

#[instrument(skip_all)]
pub fn generate_enum(
    domain: &Domain,
    union: &Union,
    variants: &[VariantContext<'_>],
) -> anyhow::Result<Vec<TokenStream>> {
    let public = &domain.public_visibility;
    let allow_unused = if domain.public_visibility.is_empty() {
        quote! {}
    } else {
        quote! { #[allow(unused)] }
    };
    let enum_name = Ident::new(union.enum_name(), Span::call_site());

    let additional_derives = {
        let mut derives = TokenStream::new();
        if !union.meta.derive.is_empty() {
            let names = union.meta.derive.iter().map(|name| Ident::new(name, Span::call_site()));
            derives = quote! { ,#(#names),* };
        }
        match &union.kind {
            UnionKind::Record(record) => {
                if record.copy {
                    derives.extend(quote! { , Clone, Copy })
                } else {
                    derives.extend(quote! { , Clone })
                }
            }
            UnionKind::Id(_) | UnionKind::BitpackedId(_) => {
                derives.extend(quote! { , Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash })
            }
        }
        derives
    };

    let docstr = proc_macro2::Literal::string(&docstr::generated_from(
        domain,
        union.span,
        union.description.as_deref(),
    ));
    let enum_variants = variants.iter().copied().map(EnumVariant);
    let union_enum = quote! {
        #[doc = #docstr]
        #[derive(serde::Serialize, serde::Deserialize #additional_derives)]
        pub #public enum #enum_name {
            #(#enum_variants),*
        }
    };

    let mut code_sections = vec![union_enum];

    let debug_variants = variants.iter().copied().map(|variant| DebugVariantBranch {
        variant,
        is_walker: false,
        enum_name: union.enum_name(),
    });
    code_sections.push(quote! {
        impl std::fmt::Debug for #enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #(#debug_variants)*
                }
            }
        }
    });

    let from_variants = variants.iter().copied().map(|variant| FromVariant {
        variant,
        enum_name: union.enum_name(),
    });
    code_sections.push(quote! { #(#from_variants)* });

    let as_variants = variants.iter().copied().map(|variant| AsVariant {
        variant,
        enum_name: union.enum_name(),
    });
    code_sections.push(quote! {
        #allow_unused
        impl #enum_name {
            #(#as_variants)*
        }
    });

    Ok(code_sections)
}

struct EnumVariant<'a>(VariantContext<'a>);

impl quote::ToTokens for EnumVariant<'_> {
    #[instrument(name = "enum_variant", skip_all, fields(variant = ?self.0.variant))]
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let variant = Ident::new(&self.0.name, Span::call_site());
        let tt = if let Some(value) = self.0.value {
            let storage_type = Ident::new(value.storage_type().name(), Span::call_site());
            quote! { #variant(#storage_type) }
        } else {
            quote! { #variant }
        };
        tokens.append_all(tt);
    }
}

struct FromVariant<'a> {
    variant: VariantContext<'a>,
    enum_name: &'a str,
}

impl quote::ToTokens for FromVariant<'_> {
    #[instrument(name = "from_variant", skip_all, fields(variant = ?self.variant.variant))]
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let enum_ = Ident::new(self.enum_name, Span::call_site());
        let variant = Ident::new(&self.variant.name, Span::call_site());
        let Some(value) = self.variant.value else {
            return;
        };
        let storage_type = Ident::new(value.storage_type().name(), Span::call_site());
        tokens.append_all(quote! {
            impl From<#storage_type> for #enum_ {
                fn from(value: #storage_type) -> Self {
                    #enum_::#variant(value)
                }
            }
        });
    }
}

struct AsVariant<'a> {
    variant: VariantContext<'a>,
    enum_name: &'a str,
}

impl quote::ToTokens for AsVariant<'_> {
    #[instrument(name = "as_walker_variant", skip_all, fields(variant = ?self.variant.variant))]
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let public = &self.variant.domain.public_visibility;
        let enum_ = Ident::new(self.enum_name, Span::call_site());
        let variant = Ident::new(&self.variant.name, Span::call_site());
        let is_variant = Ident::new(&format!("is_{}", self.variant.name.to_snake()), Span::call_site());

        if let Some(value) = self.variant.value {
            tokens.append_all(quote! {
                pub #public fn #is_variant(&self) -> bool {
                    matches!(self, #enum_::#variant(_))
                }
            });
            let as_variant = Ident::new(&format!("as_{}", self.variant.name.to_snake()), Span::call_site());
            let ty = Ident::new(value.storage_type().name(), Span::call_site());
            if value.storage_type().is_copy() {
                let val = if value.storage_type().is_id() { "id" } else { "item" };
                let val = Ident::new(val, Span::call_site());
                tokens.append_all(quote! {
                    pub #public fn #as_variant(&self) -> Option<#ty> {
                        match self {
                            #enum_::#variant(#val) => Some(*#val),
                            _ => None
                        }
                    }
                });
            } else {
                tokens.append_all(quote! {
                    pub #public fn #as_variant(&self) -> Option<&#ty> {
                        match self {
                            #enum_::#variant(item) => Some(item),
                            _ => None
                        }
                    }
                });
            }
        } else {
            tokens.append_all(quote! {
                pub #public fn #is_variant(&self) -> bool {
                    matches!(self, #enum_::#variant)
                }
            });
        }
    }
}
