use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Attribute, Data, DeriveInput, Error, Expr, ExprLit,
    Fields, Ident, Lit, LitStr,
};

#[proc_macro_derive(MetricsGroup, attributes(metrics_group))]
pub fn derive_metrics_group(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let mut out = proc_macro2::TokenStream::new();
    out.extend(expand_metrics(&input).unwrap_or_else(Error::into_compile_error));
    out.extend(expand_iterable(&input).unwrap_or_else(Error::into_compile_error));
    out.into()
}

#[proc_macro_derive(Iterable)]
pub fn derive_iterable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let out = expand_iterable(&input).unwrap_or_else(Error::into_compile_error);
    out.into()
}

fn expand_iterable(input: &DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let (name, fields) = parse_named_struct(input)?;

    let count = fields.len();

    let mut match_arms = quote! {};
    for (i, field) in fields.iter().enumerate() {
        let ident = field.ident.as_ref().unwrap();
        let ident_str = ident.to_string();
        match_arms.extend(quote! {
            #i => Some((#ident_str, &self.#ident as &dyn ::std::any::Any)),
        });
    }

    Ok(quote! {
        impl ::iroh_metrics::Iterable for #name {
            fn field_count(&self) -> usize {
                #count
            }

            fn field(&self, n: usize) -> Option<(&'static str, &dyn ::std::any::Any)> {
                match n {
                    #match_arms
                    _ => None,
                }
            }
        }
    })
}

fn expand_metrics(input: &DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let (name, fields) = parse_named_struct(input)?;

    let mut field_defaults = quote! {};
    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        let description = parse_doc_first_line(&field.attrs);
        let description = description.unwrap_or_else(|| field_name.to_string());

        field_defaults.extend(quote! {
            #field_name: #ty::new(#description),
        });
    }

    let name_str = parse_metrics_name(&input.attrs)?;
    let name_str = name_str.unwrap_or_else(|| name.to_string().to_snake_case());

    Ok(quote! {
        impl ::std::default::Default for #name {
            fn default() -> Self {
                Self {
                    #field_defaults
                }
            }
        }

        impl ::iroh_metrics::MetricsGroup for #name {
            fn name(&self) -> &'static str {
                #name_str
            }
        }
    })
}

fn parse_doc_first_line(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .flat_map(|attr| attr.meta.require_name_value())
        .find_map(|name_value| {
            let Expr::Lit(ExprLit { lit, .. }) = &name_value.value else {
                return None;
            };
            let Lit::Str(str) = lit else { return None };
            Some(str.value().trim().to_string())
        })
}

fn parse_metrics_name(attrs: &[Attribute]) -> Result<Option<String>, syn::Error> {
    let mut out = None;
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("metrics_group"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let s: LitStr = meta.value()?.parse()?;
                out = Some(s.value().trim().to_string());
                Ok(())
            } else {
                Err(meta
                    .error("The `metrics_group` attribute supports only a single `name` value. "))
            }
        })?;
    }
    Ok(out)
}

fn parse_named_struct(input: &DeriveInput) -> Result<(&Ident, &Fields), Error> {
    match &input.data {
        Data::Struct(data) if matches!(data.fields, Fields::Named(_)) => {
            Ok((&input.ident, &data.fields))
        }
        _ => Err(Error::new(
            input.span(),
            "The `MetricsGroup` and `Iterable` derives support only structs.",
        )),
    }
}
