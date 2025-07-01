//! Derives for SQL helper
//!

use quote::{quote, quote_spanned};
use syn::{
    Data, DeriveInput, Fields, GenericParam, Generics, Type, TypeParamBound, parse_macro_input,
    parse_quote, spanned::Spanned,
};

/// Derive `FromRow`.
#[proc_macro_derive(FromRow)]
pub fn derive_from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    // Add required trait bounds depending on type.
    let generics = add_trait_bounds(
        input.generics,
        parse_quote!(ts_sql_helper_lib::postgres::types::FromSql),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let Data::Struct(data_struct) = input.data else {
        panic!("FromRow can only be derived on a struct")
    };

    let Fields::Named(fields) = data_struct.fields else {
        panic!("FromRow can only be derived on a struct with named fields")
    };

    let each_field_from_row = fields.named.iter().filter_map(|f| {
        let name = f.ident.as_ref()?;

        let field_type = &f.ty;

        let name_lit = name.to_string();

        Some(quote_spanned! {f.span()=>
            let #name: #field_type = row.try_get(#name_lit).ok()?;
        })
    });

    let struct_fields = fields.named.iter().map(|f| {
        let name = &f.ident;
        quote_spanned! {f.span() => #name}
    });

    let expanded = quote! {
        // The generated impl.
        impl #impl_generics ts_sql_helper_lib::FromRow for #name #ty_generics #where_clause {
            fn from_row(row: &ts_sql_helper_lib::postgres::Row) -> Option<Self> {
                #( #each_field_from_row )*

                Some(Self {
                    #( #struct_fields ),*
                })
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

/// Derive `FromSql`
#[proc_macro_derive(FromSql)]
pub fn derive_from_sql(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    if !matches!(input.data, Data::Enum(_)) {
        panic!("FromSql can only be derived on an enum")
    }

    let name = input.ident;

    let (repr, accepts, from_sql) = {
        let mut repr_type = parse_quote!(&str);
        let mut accepts = quote!(ts_sql_helper_lib::postgres::types::accepts!(TEXT));
        let mut from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::text_from_sql(
            raw
        )?);

        for attr in input.attrs {
            if !attr.path().is_ident("repr") {
                continue;
            }

            let Ok(arg) = attr.parse_args::<Type>() else {
                continue;
            };

            if arg == parse_quote!(i8) {
                accepts = quote!(ts_sql_helper_lib::postgres::types::accepts!(CHAR));
                from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::char_from_sql(
                    raw
                )?);
            } else if arg == parse_quote!(i16) {
                accepts = quote!(ts_sql_helper_lib::postgres::types::accepts!(INT2));
                from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::int2_from_sql(
                    raw
                )?);
            } else if arg == parse_quote!(i32) {
                accepts = quote!(ts_sql_helper_lib::postgres::types::accepts!(INT4));
                from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::int4_from_sql(
                    raw
                )?);
            } else if arg == parse_quote!(i64) {
                accepts = quote!(ts_sql_helper_lib::postgres::types::accepts!(INT8));
                from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::int8_from_sql(
                    raw
                )?);
            } else {
                continue;
            }

            repr_type = arg;
            break;
        }

        (repr_type, accepts, from_sql)
    };

    let generics = add_trait_bounds(input.generics, parse_quote!(TryFrom<#repr>));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        impl<'a> #impl_generics ts_sql_helper_lib::postgres::types::FromSql<'a> for #name #ty_generics #where_clause {
            fn from_sql(_: &ts_sql_helper_lib::postgres::types::Type, raw: &[u8]) -> Result<Self, Box<dyn core::error::Error + Sync + Send>> {
                let raw_value = #from_sql;
                let value = Self::try_from(raw_value)?;
                Ok(value)
            }

            #accepts;

        }
    };

    proc_macro::TokenStream::from(expanded)
}

// Add a bound to every type parameter T.
fn add_trait_bounds(mut generics: Generics, bounds: TypeParamBound) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(bounds.clone());
        }
    }
    generics
}
