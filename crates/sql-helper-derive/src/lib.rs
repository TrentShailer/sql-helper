//! Derives for SQL helper
//!

use std::sync::LazyLock;

use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use regex::Regex;
use syn::{
    Data, DeriveInput, Fields, GenericParam, Generics, Ident, LitInt, LitStr, Token, Type,
    TypeParamBound, bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    spanned::Spanned,
};

struct QueryMacroInput {
    name: Ident,
    query: LitStr,
    optional_params: Vec<usize>,
}
impl Parse for QueryMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.parse::<Ident>()? != Ident::new("name", input.span()) {
            return Err(input.error("expected `name`"));
        }
        input.parse::<Token![:]>()?;
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut ident = input.parse::<Ident>()?;
        let optional_params = if ident == Ident::new("optional_params", input.span()) {
            input.parse::<Token![:]>()?;

            let content;
            bracketed![content in input];
            let optional_params: Vec<_> = content
                .parse_terminated(LitInt::parse, Token![,])?
                .iter()
                .map(|v| v.base10_parse().unwrap())
                .collect();

            input.parse::<Token![,]>()?;

            ident = input.parse::<Ident>()?;
            optional_params
        } else {
            Vec::new()
        };

        if ident != Ident::new("query", input.span()) {
            return Err(input.error("expected `query`"));
        }
        input.parse::<Token![:]>()?;
        let query: LitStr = input.parse()?;

        Ok(Self {
            name,
            query,
            optional_params,
        })
    }
}

/// Macro for creating and test SQL.
#[proc_macro]
pub fn query(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as QueryMacroInput);

    pub enum State {
        Neutral,
        ConsumingNumber { has_consumed_a_digit: bool },
        ConsumingTypeSeparator,
        ConsumingType { type_string: String },
    }

    let query = input.query.value();
    static REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)(\r\n|\r|\n| ){2,}").unwrap());
    let query = REGEX.replace_all(query.trim(), " ");

    let mut parameter_types = vec![];
    let mut state = State::Neutral;
    for character in query.chars() {
        match &mut state {
            State::Neutral => {
                if character == '$' {
                    state = State::ConsumingNumber {
                        has_consumed_a_digit: false,
                    };
                }
            }
            State::ConsumingNumber {
                has_consumed_a_digit,
            } => {
                if character.is_ascii_digit() {
                    *has_consumed_a_digit = true;
                } else if character == ':' {
                    state = State::ConsumingTypeSeparator;
                } else {
                    if *has_consumed_a_digit {
                        parameter_types.push("unknown".to_string());
                    }
                    state = State::Neutral;
                }
            }
            State::ConsumingTypeSeparator => {
                if character.is_ascii_alphabetic() {
                    state = State::ConsumingType {
                        type_string: character.to_string(),
                    };
                } else if character != ':' {
                    parameter_types.push("unknown".to_string());
                    state = State::Neutral;
                }
            }
            State::ConsumingType { type_string } => {
                if character.is_ascii_alphabetic() || character == '[' || character == ']' {
                    type_string.push(character);
                } else {
                    parameter_types.push(type_string.to_uppercase());
                    state = State::Neutral;
                }
            }
        }
    }
    match state {
        State::Neutral => {}
        State::ConsumingNumber {
            has_consumed_a_digit,
        } => {
            if has_consumed_a_digit {
                parameter_types.push("unknown".to_string());
            }
        }
        State::ConsumingTypeSeparator => {
            parameter_types.push("unknown".to_string());
        }
        State::ConsumingType { type_string } => {
            parameter_types.push(type_string.to_uppercase());
        }
    }

    let struct_name = input.name;
    let param_struct_name = format_ident!("{struct_name}Params");
    let param_count = parameter_types.len();

    const KNOWN_TYPES: [&str; 30] = [
        "BOOL",
        "BOOL[]",
        "BYTEA",
        "BYTEA[]",
        "CHAR",
        "CHAR[]",
        "INT8",
        "INT8[]",
        "INT4",
        "INT4[]",
        "INT2",
        "INT2[]",
        "FLOAT8",
        "FLOAT8[]",
        "FLOAT4",
        "FLOAT4[]",
        "UUID",
        "UUID[]",
        "TEXT",
        "VARCHAR",
        "VARCHAR[]",
        "TEXT[]",
        "TIMESTAMP",
        "TIMESTAMP[]",
        "TIMESTAMPTZ",
        "TIMESTAMPTZ[]",
        "DATE",
        "DATE[]",
        "TIME",
        "TIME[]",
    ];
    let param_types: Vec<Type> = parameter_types
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let param_number = index + 1;
            let param_type = match name.as_str() {
                "BOOL" => parse_quote!(&'a bool),
                "BOOL[]" => parse_quote!(&'a [bool]),
                "BYTEA" => parse_quote!(&'a [u8]),
                "BYTEA[]" => parse_quote!(&'a [Vec<u8>]),
                "CHAR" => parse_quote!(&'a i8),
                "CHAR[]" => parse_quote!(&'a [i8]),
                "INT8" => parse_quote!(&'a i64),
                "INT8[]" => parse_quote!(&'a [i64]),
                "INT4" => parse_quote!(&'a i32),
                "INT4[]" => parse_quote!(&'a [i32]),
                "INT2" => parse_quote!(&'a i16),
                "INT2[]" => parse_quote!(&'a [i16]),
                "FLOAT8" => parse_quote!(&'a f64),
                "FLOAT8[]" => parse_quote!(&'a [f64]),
                "FLOAT4" => parse_quote!(&'a f32),
                "FLOAT4[]" => parse_quote!(&'a [f32]),
                "UUID" => parse_quote!(&'a uuid::Uuid),
                "UUID[]" => parse_quote!(&'a [uuid::Uuid]),
                "TEXT" | "VARCHAR" => parse_quote!(&'a str),
                "VARCHAR[]" | "TEXT[]" => parse_quote!(&'a [String]),
                "TIMESTAMP" => parse_quote!(&'a ts_sql_helper_lib::SqlDateTime),
                "TIMESTAMP[]" => parse_quote!(&'a [ts_sql_helper_lib::SqlDateTime]),
                "TIMESTAMPTZ" => parse_quote!(&'a ts_sql_helper_lib::SqlTimestamp),
                "TIMESTAMPTZ[]" => parse_quote!(&'a [ts_sql_helper_lib::SqlTimestamp]),
                "DATE" => parse_quote!(&'a ts_sql_helper_lib::SqlDate),
                "DATE[]" => parse_quote!(&'a [ts_sql_helper_lib::SqlDate]),
                "TIME" => parse_quote!(&'a ts_sql_helper_lib::SqlTime),
                "TIME[]" => parse_quote!(&'a [ts_sql_helper_lib::SqlTime]),

                _ => parse_quote!(&'a (dyn ts_sql_helper_lib::postgres::types::ToSql + Sync)),
            };
            if input.optional_params.contains(&param_number) {
                parse_quote!(Option<#param_type>)
            } else {
                param_type
            }
        })
        .collect();
    let param_names: Vec<Ident> = (1..param_count + 1)
        .map(|number| format_ident!("p{number}"))
        .collect();

    let params: Vec<_> = param_types
        .iter()
        .enumerate()
        .map(|(index, field_type)| {
            let name = &param_names[index];
            quote! {
                #name: #field_type
            }
        })
        .collect();

    let pub_params = params.iter().map(|param| quote! {pub #param});
    let self_params = param_names.iter().enumerate().map(|(index, param)| {
        let type_string = &parameter_types[index];
        if KNOWN_TYPES.contains(&type_string.as_str()) {
            quote!(&self.#param)
        } else {
            quote!(self.#param)
        }
    });

    let test_name = format_ident!("test_{struct_name}");
    let test = quote! {
        #[cfg(test)]
        #[allow(non_snake_case)]
        #[test]
        fn #test_name() {
            use ts_sql_helper_lib::test::get_test_database;

            let (mut client, _container) = get_test_database();
            let statement = client.prepare(#struct_name::QUERY);
            assert!(statement.is_ok(), "invalid query `{}`: {}", #struct_name::QUERY, statement.unwrap_err());
            let statement = statement.unwrap();

            let mut data: Vec<Box<dyn ts_sql_helper_lib::postgres_types::ToSql + Sync>> = Vec::new();
            let params = statement.params();
            for param in params.iter() {
                match ts_sql_helper_lib::test::data_for_type(param) {
                    Some(param_data) => data.push(param_data),
                    None => panic!("unsupported parameter type `{}`", param.name()),
                }
            }

            let borrowed_data: Vec<&(dyn ts_sql_helper_lib::postgres_types::ToSql + Sync)> =
                data.iter().map(|data| data.as_ref()).collect();

            let result = client.execute(&statement, borrowed_data.as_slice());
            if let Err(error) = result {
                use ts_sql_helper_lib::postgres::error::SqlState;

                assert!(
                    matches!(
                        error.code(),
                        Some(&SqlState::FOREIGN_KEY_VIOLATION) | Some(&SqlState::CHECK_VIOLATION)
                    ),
                    "invalid query `{}`: {error}",
                    #struct_name::QUERY
                );
            }
        }
    };
    quote! {
        struct #struct_name;
        impl #struct_name {
            pub const QUERY: &str = #query;
            pub fn params<'a>(#( #params ),*) -> #param_struct_name<'a> {
                #param_struct_name {
                    #( #param_names , )*
                    phantom_data: core::marker::PhantomData,
                }
            }
        }
        struct #param_struct_name<'a> {
            #( #pub_params , )*
            pub phantom_data: core::marker::PhantomData<&'a ()>,
        }
        impl<'a>  #param_struct_name<'a> {
            pub fn as_array(&'a self) -> [&'a (dyn ts_sql_helper_lib::postgres::types::ToSql + Sync); #param_count] {
                [
                    #( #self_params , )*
                ]
            }
        }
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
