use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, DeriveInput, Expr, ExprLit, Fields, Lit, Meta, MetaList, MetaNameValue,
    parse::Parser, parse_macro_input,
};

#[proc_macro_derive(MetricsGroup, attributes(metrics))]
pub fn derive_metrics_group(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_metrics(&input).into()
}

fn impl_metrics(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;

    let syn::Data::Struct(data) = &input.data else {
        panic!("Only structs are supported.")
    };
    let Fields::Named(_fields) = &data.fields else {
        panic!("Only structs with named fields are supported.")
    };

    let mut fields_impl = quote! {};

    for field in &data.fields {
        let field_name = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        let doc = parse_doc_comment(&field.attrs)
            .first()
            .cloned()
            .unwrap_or_else(|| field_name.to_string());
        fields_impl = quote! {
            #fields_impl
            #field_name: #ty::new(#doc),
        };
    }

    let attr_name =
        parse_metrics_name(&input.attrs).unwrap_or_else(|| name.to_string().to_snake_case());

    quote! {
        impl Default for #name {
            fn default() -> Self {
                Self {
                    #fields_impl
                }
            }
        }

        impl MetricsGroup for #name {
            fn name(&self) -> &'static str {
                #attr_name
            }
        }
    }
}

fn parse_doc_comment(attrs: &[Attribute]) -> Vec<String> {
    let mut lines = vec![];
    for attr in attrs {
        if let Meta::NameValue(MetaNameValue { path, value, .. }) = &attr.meta {
            if path.is_ident("doc") {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit_str),
                    ..
                }) = value
                {
                    lines.push(lit_str.value().trim().to_string());
                }
            }
        }
    }
    lines
}

fn parse_metrics_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if let Meta::List(MetaList { path, tokens, .. }) = &attr.meta {
            if path.is_ident("metrics") {
                let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                if let Ok(parsed) = parser.parse2(tokens.clone()) {
                    for meta in parsed {
                        if let Meta::NameValue(MetaNameValue {
                            path,
                            value:
                                Expr::Lit(ExprLit {
                                    lit: Lit::Str(lit_str),
                                    ..
                                }),
                            ..
                        }) = meta
                        {
                            if path.is_ident("name") {
                                return Some(lit_str.value());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
