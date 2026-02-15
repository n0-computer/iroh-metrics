use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Error, Expr, ExprLit, Fields, GenericArgument, Ident, Lit,
    LitStr, PathArguments, Type, meta::ParseNestedMeta, parse_macro_input, spanned::Spanned,
};

#[proc_macro_derive(MetricsGroup, attributes(metrics, default))]
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

#[proc_macro_derive(MetricsGroupSet, attributes(metrics))]
pub fn derive_metrics_group_set(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let mut out = proc_macro2::TokenStream::new();
    out.extend(expand_metrics_group_set(&input).unwrap_or_else(Error::into_compile_error));
    out.into()
}

#[proc_macro_derive(EncodeLabelSet, attributes(label))]
pub fn derive_encode_label_set(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_encode_label_set(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_iterable(input: &DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let (name, fields) = parse_named_struct(input)?;

    // Separate regular metrics and Family fields
    let (metric_fields, family_fields): (Vec<_>, Vec<_>) =
        fields.iter().partition(|f| !is_family_type(&f.ty));

    let metric_count = metric_fields.len();
    let family_count = family_fields.len();

    // Generate match arms for regular metrics
    let mut metric_match_arms = quote! {};
    for (i, field) in metric_fields.iter().enumerate() {
        let ident = field.ident.as_ref().unwrap();
        let ident_str = ident.to_string();
        let attr = parse_metrics_attr(&field.attrs)?;
        let help = attr
            .help
            .or_else(|| parse_doc_first_line(&field.attrs))
            .unwrap_or_else(|| ident_str.clone());
        metric_match_arms.extend(quote! {
            #i => Some(::iroh_metrics::MetricItem::new(#ident_str, #help, &self.#ident as &dyn ::iroh_metrics::Metric)),
        });
    }

    // Generate match arms for Family fields
    let mut family_match_arms = quote! {};
    for (i, field) in family_fields.iter().enumerate() {
        let ident = field.ident.as_ref().unwrap();
        let ident_str = ident.to_string();
        let attr = parse_metrics_attr(&field.attrs)?;
        let help = attr
            .help
            .or_else(|| parse_doc_first_line(&field.attrs))
            .unwrap_or_else(|| ident_str.clone());
        family_match_arms.extend(quote! {
            #i => Some(::iroh_metrics::FamilyItem::new(#ident_str, #help, &self.#ident as &dyn ::iroh_metrics::FamilyEncoder)),
        });
    }

    let family_impl = if family_count > 0 {
        quote! {
            fn family_count(&self) -> usize {
                #family_count
            }

            fn family_ref(&self, n: usize) -> Option<::iroh_metrics::FamilyItem<'_>> {
                match n {
                    #family_match_arms
                    _ => None,
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl ::iroh_metrics::iterable::Iterable for #name {
            fn field_count(&self) -> usize {
                #metric_count
            }

            fn field_ref(&self, n: usize) -> Option<::iroh_metrics::MetricItem<'_>> {
                match n {
                    #metric_match_arms
                    _ => None,
                }
            }

            #family_impl
        }
    })
}

/// Checks if a type is `Family<_, _>`.
fn is_family_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Family" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    // Family should have 2 generic arguments
                    return args
                        .args
                        .iter()
                        .filter(|arg| matches!(arg, GenericArgument::Type(_)))
                        .count()
                        == 2;
                }
            }
        }
    }
    false
}

fn expand_metrics(input: &DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let (name, fields) = parse_named_struct(input)?;
    let attr = parse_metrics_attr(&input.attrs)?;
    let name_str = attr
        .name
        .unwrap_or_else(|| name.to_string().to_snake_case());

    let default = if attr.default {
        let mut items = vec![];
        for field in fields.iter() {
            let ident = field
                .ident
                .as_ref()
                .ok_or_else(|| Error::new(field.span(), "Only named fields are supported"))?;
            let attr = parse_default_attr(&field.attrs)?;
            let item = if let Some(expr) = attr {
                quote!( #ident: #expr)
            } else {
                quote!( #ident: ::std::default::Default::default() )
            };
            items.push(item);
        }
        Some(quote! {
            impl ::std::default::Default for #name {
                fn default() -> Self {
                    Self {
                        #(#items),*
                    }
                }
            }
        })
    } else {
        None
    };

    Ok(quote! {
        impl ::iroh_metrics::MetricsGroup for #name {
            fn name(&self) -> &'static str {
                #name_str
            }
        }

        #default
    })
}

fn expand_metrics_group_set(input: &DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let (name, fields) = parse_named_struct(input)?;
    let attr = parse_metrics_attr(&input.attrs)?;
    let name_str = attr
        .name
        .unwrap_or_else(|| name.to_string().to_snake_case());

    let mut cloned = quote! {};
    let mut refs = quote! {};
    for field in fields {
        let name = field.ident.as_ref().unwrap();
        cloned.extend(quote! {
            self.#name.clone() as ::std::sync::Arc<dyn ::iroh_metrics::MetricsGroup>,
        });
        refs.extend(quote! {
            &*self.#name as &dyn ::iroh_metrics::MetricsGroup,
        });
    }

    Ok(quote! {
        impl ::iroh_metrics::MetricsGroupSet for #name {
            fn name(&self) -> &'static str {
                #name_str
            }

            fn groups_cloned(&self) -> impl ::std::iter::Iterator<Item = ::std::sync::Arc<dyn ::iroh_metrics::MetricsGroup>> {
                [#cloned].into_iter()
            }

            fn groups(&self) -> impl ::std::iter::Iterator<Item = &dyn ::iroh_metrics::MetricsGroup> {
                [#refs].into_iter()
            }
        }
    })
}

fn expand_encode_label_set(input: &DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let (name, fields) = parse_named_struct(input)?;

    let mut label_pairs = vec![];
    for field in fields.iter() {
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| Error::new(field.span(), "Only named fields are supported"))?;
        let attr = parse_label_attr(&field.attrs)?;

        // Skip fields marked with #[label(skip)]
        if attr.skip {
            continue;
        }

        // Use custom name or field name
        let label_name = attr.name.unwrap_or_else(|| ident.to_string());

        // Generate the label pair based on field type
        label_pairs.push(quote! {
            (#label_name, ::iroh_metrics::LabelValue::from(self.#ident.clone()))
        });
    }

    Ok(quote! {
        impl ::iroh_metrics::EncodeLabelSet for #name {
            fn encode_label_pairs(&self) -> ::std::vec::Vec<::iroh_metrics::LabelPair<'_>> {
                ::std::vec![#(#label_pairs),*]
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

#[derive(Default)]
struct MetricsAttr {
    name: Option<String>,
    help: Option<String>,
    default: bool,
}

#[derive(Default)]
struct LabelAttr {
    name: Option<String>,
    skip: bool,
}

fn parse_default_attr(attrs: &[Attribute]) -> Result<Option<syn::Expr>, syn::Error> {
    let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident("default")) else {
        return Ok(None);
    };
    let expr = attr.parse_args::<Expr>()?;
    Ok(Some(expr))
}

fn parse_metrics_attr(attrs: &[Attribute]) -> Result<MetricsAttr, syn::Error> {
    let mut out = MetricsAttr::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("metrics")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                out.name = Some(parse_lit_str(&meta)?);
                Ok(())
            } else if meta.path.is_ident("help") {
                out.help = Some(parse_lit_str(&meta)?);
                Ok(())
            } else if meta.path.is_ident("default") {
                out.default = true;
                Ok(())
            } else {
                Err(meta.error(
                    "The `metrics` attribute supports only `name`, `help` and `default` fields.",
                ))
            }
        })?;
    }
    Ok(out)
}

fn parse_label_attr(attrs: &[Attribute]) -> Result<LabelAttr, syn::Error> {
    let mut out = LabelAttr::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("label")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                out.name = Some(parse_lit_str(&meta)?);
                Ok(())
            } else if meta.path.is_ident("skip") {
                out.skip = true;
                Ok(())
            } else {
                Err(meta.error("The `label` attribute supports only `name` and `skip` fields."))
            }
        })?;
    }
    Ok(out)
}

fn parse_lit_str(meta: &ParseNestedMeta<'_>) -> Result<String, Error> {
    let s: LitStr = meta.value()?.parse()?;
    Ok(s.value().trim().to_string())
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
