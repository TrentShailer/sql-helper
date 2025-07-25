//! Derives for SQL helper
//!

use std::sync::LazyLock;

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use regex::Regex;
use syn::{
    Data, DeriveInput, Fields, GenericParam, Generics, Type, TypeParamBound, parse_macro_input,
    parse_quote, spanned::Spanned,
};

use crate::query::{
    QueryMacroInput,
    main_struct::create_main_struct,
    parameters::{get_param_types, parameter_to_type},
    row_struct::create_row_struct,
    test::create_test,
};

mod query;

/// Macro for creating and test SQL.
#[proc_macro]
pub fn query(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as QueryMacroInput);

    let query = input.query.value();
    static REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)(\r\n|\r|\n| ){2,}").unwrap());
    let query = REGEX.replace_all(query.trim(), " ");

    let parameters: Vec<Type> = get_param_types(&query)
        .into_iter()
        .enumerate()
        .map(|(index, parameter)| {
            let r#type = parameter_to_type(&parameter);
            if input
                .optional_params
                .as_ref()
                .is_some_and(|params| params.contains(&(index + 1)))
            {
                parse_quote!(Option<#r#type>)
            } else {
                r#type
            }
        })
        .collect();

    let struct_name = input.name;

    let main_struct = create_main_struct(&struct_name, &query, &parameters);
    let test = create_test(&struct_name);
    let row_struct = if let Some(row_fields) = input.row {
        create_row_struct(&struct_name, &row_fields)
    } else {
        proc_macro2::TokenStream::new()
    };

    quote! {
        #main_struct
        #row_struct
        #test
    }
    .into()
}

/// Derive `FromRow`.
#[proc_macro_derive(FromRow)]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
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
        let name_lit = name.to_string();
        let field_type = &f.ty;

        Some(quote_spanned! {f.span()=>
            let #name: #field_type = row.try_get(#name_lit)?;
        })
    });

    let struct_fields = fields.named.iter().map(|f| {
        let name = &f.ident;
        quote_spanned! {f.span() => #name}
    });

    let expanded = quote! {
        // The generated impl.
        impl #impl_generics ts_sql_helper_lib::FromRow for #name #ty_generics #where_clause {
            fn from_row(row: &ts_sql_helper_lib::postgres::Row) -> Result<Self, ts_sql_helper_lib::postgres::Error> {
                #( #each_field_from_row )*

                Ok(Self {
                    #( #struct_fields ),*
                })
            }
        }
    };

    // Hand the output tokens back to the compiler.
    TokenStream::from(expanded)
}

/// Derive `FromSql`
#[proc_macro_derive(FromSql)]
pub fn derive_from_sql(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    if !matches!(input.data, Data::Enum(_)) {
        panic!("FromSql can only be derived on an enum")
    }

    let name = input.ident;

    let (repr, accepts, from_sql) = {
        let mut repr_type = parse_quote!(&str);
        let mut accepts: Vec<Type> = vec![
            parse_quote!(ts_sql_helper_lib::postgres_types::Type::TEXT),
            parse_quote!(ts_sql_helper_lib::postgres_types::Type::VARCHAR),
        ];
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
                accepts = vec![parse_quote!(ts_sql_helper_lib::postgres_types::Type::CHAR)];
                from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::char_from_sql(
                    raw
                )?);
            } else if arg == parse_quote!(i16) {
                accepts = vec![parse_quote!(ts_sql_helper_lib::postgres_types::Type::INT2)];
                from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::int2_from_sql(
                    raw
                )?);
            } else if arg == parse_quote!(i32) {
                accepts = vec![parse_quote!(ts_sql_helper_lib::postgres_types::Type::INT4)];
                from_sql = quote!(ts_sql_helper_lib::postgres_protocol::types::int4_from_sql(
                    raw
                )?);
            } else if arg == parse_quote!(i64) {
                accepts = vec![parse_quote!(ts_sql_helper_lib::postgres_types::Type::INT8)];
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

            fn accepts(ty: &ts_sql_helper_lib::postgres_types::Type) -> bool {
                match (*ty) {
                    #(#accepts)|* => true,
                    _ => false,
                }
            }
        }
    };

    TokenStream::from(expanded)
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
