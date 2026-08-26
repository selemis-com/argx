//! Code generation for standalone Rust type contracts.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericParam, Generics, Type, parse_quote};

use crate::{attrs, crate_name};

/// Emits a standalone type-contract resolver for one struct or enum declaration.
pub(crate) fn contract(input: &DeriveInput) -> syn::Result<TokenStream> {
    let body = match &input.data {
        Data::Struct(data) => struct_kind(&data.fields),
        Data::Enum(data) => enum_kind(data),
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "Contract can only be derived for structs and enums",
            ));
        }
    };

    let facade = crate_name::facade_path();
    let ident = &input.ident;
    let name = ident_name(ident);
    let description = option_str(attrs::doc_summary(&input.attrs).as_deref());
    let generics = contract_generics(&input.generics, &facade);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let type_keys = input.generics.params.iter().filter_map(|parameter| match parameter {
        GenericParam::Type(parameter) => {
            let ident = &parameter.ident;
            Some(quote!(<#ident as #facade::__private::TypeContractSource>::type_key()))
        }
        GenericParam::Lifetime(_) | GenericParam::Const(_) => None,
    });
    let const_keys = input.generics.params.iter().filter_map(|parameter| match parameter {
        GenericParam::Const(parameter) => {
            let ident = &parameter.ident;
            Some(quote!(#facade::__private::const_key(&#ident)))
        }
        GenericParam::Lifetime(_) | GenericParam::Type(_) => None,
    });

    Ok(quote! {
        #[doc(hidden)]
        impl #impl_generics #facade::__private::TypeContractSource for #ident #ty_generics
        #where_clause
        {
            fn resolve_type(
                resolver: &mut #facade::__private::TypeResolver,
            ) -> #facade::TypeContractValue {
                resolver.named(
                    <Self as #facade::__private::TypeContractSource>::type_key(),
                    #name,
                    #description,
                    |resolver| #body,
                )
            }

            fn type_key() -> #facade::__private::TypeKey {
                #[doc = "Private nominal type-contract declaration marker."]
                struct Marker;
                #facade::__private::TypeKey::named::<Marker>(
                    ::std::vec![#(#type_keys),*],
                    ::std::vec![#(#const_keys),*],
                )
            }
        }
    })
}

/// Adds the public contract bound needed to resolve and identify generic type arguments.
fn contract_generics(generics: &Generics, facade: &TokenStream) -> Generics {
    let mut bounded = generics.clone();
    for parameter in bounded.type_params_mut() {
        parameter.bounds.push(parse_quote!(#facade::ContractType));
    }
    bounded
}

/// Emits the structural kind of one struct declaration.
fn struct_kind(fields: &Fields) -> TokenStream {
    let facade = crate_name::facade_path();
    match fields {
        Fields::Named(fields) => {
            let fields = fields.named.iter().map(named_field);
            quote!(#facade::TypeDefinitionKind::Struct { fields: ::std::vec![#(#fields),*] })
        }
        Fields::Unnamed(fields) => {
            let fields = fields.unnamed.iter().map(tuple_field);
            quote!(#facade::TypeDefinitionKind::TupleStruct { fields: ::std::vec![#(#fields),*] })
        }
        Fields::Unit => quote!(#facade::TypeDefinitionKind::UnitStruct),
    }
}

/// Emits the structural kind of one enum declaration.
fn enum_kind(data: &syn::DataEnum) -> TokenStream {
    let facade = crate_name::facade_path();
    let variants = data.variants.iter().map(|variant| {
        let name = ident_name(&variant.ident);
        let description = option_str(attrs::doc_summary(&variant.attrs).as_deref());
        let kind = match &variant.fields {
            Fields::Unit => quote!(#facade::TypeVariantKind::Unit),
            Fields::Unnamed(fields) => {
                let fields = fields.unnamed.iter().map(tuple_field);
                quote!(#facade::TypeVariantKind::Tuple { fields: ::std::vec![#(#fields),*] })
            }
            Fields::Named(fields) => {
                let fields = fields.named.iter().map(named_field);
                quote!(#facade::TypeVariantKind::Struct { fields: ::std::vec![#(#fields),*] })
            }
        };
        quote! {
            #facade::TypeVariantContract {
                name: ::std::string::String::from(#name),
                description: #description.map(::std::string::String::from),
                kind: #kind,
            }
        }
    });
    quote!(#facade::TypeDefinitionKind::Enum { variants: ::std::vec![#(#variants),*] })
}

/// Emits one field with its Rust identifier preserved as the public semantic name.
fn named_field(field: &syn::Field) -> TokenStream {
    let facade = crate_name::facade_path();
    let name = field.ident.as_ref().map(ident_name).expect("named field has an identifier");
    field_contract(&field.ty, Some(&name), attrs::doc_summary(&field.attrs).as_deref(), &facade)
}

/// Emits one unnamed tuple field.
fn tuple_field(field: &syn::Field) -> TokenStream {
    let facade = crate_name::facade_path();
    field_contract(&field.ty, None, attrs::doc_summary(&field.attrs).as_deref(), &facade)
}

/// Emits one field contract and recursively resolves its Rust type.
fn field_contract(
    ty: &Type,
    name: Option<&str>,
    description: Option<&str>,
    facade: &TokenStream,
) -> TokenStream {
    let name = option_str(name);
    let description = option_str(description);
    quote! {
        #facade::TypeFieldContract {
            name: #name.map(::std::string::String::from),
            description: #description.map(::std::string::String::from),
            value_type: <#ty as #facade::__private::TypeContractSource>::resolve_type(resolver),
        }
    }
}

/// Emits an optional borrowed string in generated code.
fn option_str(value: Option<&str>) -> TokenStream {
    value.map_or_else(
        || quote!(::std::option::Option::<&'static str>::None),
        |value| quote!(::std::option::Option::Some(#value)),
    )
}

/// Returns a Rust identifier without the raw-identifier prefix.
fn ident_name(ident: &syn::Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}
