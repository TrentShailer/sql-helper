//! Derives for SQL helper
//!

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    Data, DeriveInput, Fields, GenericParam, Generics, parse_macro_input, parse_quote,
    spanned::Spanned,
};

/// Derive `FromRow`.
#[proc_macro_derive(FromRow)]
pub fn derive_from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    // Add a bound `T: FromSql` to every type parameter T.
    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let implementation = all_from_row(&input.data);

    let expanded = quote! {
        // The generated impl.
        impl #impl_generics ts_sql_helper_lib::FromRow for #name #ty_generics #where_clause {
            fn from_row(row: &ts_sql_helper_lib::postgres::Row) -> Option<Self> {
                #implementation
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

// Add a bound `T: FromSql` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param
                .bounds
                .push(parse_quote!(ts_sql_helper_lib::postgres::types::FromSql));
        }
    }
    generics
}

fn all_from_row(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let try_get = fields.named.iter().filter_map(|f| {
                    let Some(name) = &f.ident else {
                        return None;
                    };

                    let field_type = &f.ty;

                    let name_lit = name.to_string();

                    let tokens = quote_spanned! {f.span()=>
                        let #name: #field_type = row.try_get(#name_lit).ok()?;
                    };

                    Some(tokens)
                });

                let build_self = fields.named.iter().map(|f| {
                    let name = &f.ident;

                    quote_spanned! {f.span() => #name}
                });

                quote! {
                    #( #try_get )*

                    Some(Self {
                        #( #build_self ),*
                    })
                }
            }
            Fields::Unnamed(ref fields) => {
                panic!("`#[derive(FromRow)]` is only implemented for structs with named fields")
            }
            Fields::Unit => {
                panic!("`#[derive(FromRow)]` is only implemented for structs with named fields")
            }
        },
        Data::Enum(_) | Data::Union(_) => {
            panic!("`#[derive(FromRow)]` is only implemented for structs with named fields")
        }
    }
}
