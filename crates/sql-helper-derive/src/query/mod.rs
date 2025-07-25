use quote::{ToTokens, quote};
use syn::{
    Ident, LitInt, LitStr, Token, Type, braced, bracketed,
    parse::{Parse, ParseStream},
};

pub mod main_struct;
pub mod parameters;
pub mod row_struct;
pub mod test;

pub struct RowField {
    pub name: Ident,
    pub r#type: Type,
}
impl Parse for RowField {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let r#type: Type = input.parse()?;

        Ok(Self { name, r#type })
    }
}
impl ToTokens for RowField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let r#type = &self.r#type;
        let new_tokens = quote! {
            #name: #r#type
        };

        tokens.extend(new_tokens);
    }
}

pub struct QueryMacroInput {
    pub name: Ident,
    pub row: Option<Vec<RowField>>,
    pub optional_params: Option<Vec<usize>>,
    pub query: LitStr,
}
impl Parse for QueryMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = {
            input.parse::<name_keyword::name>()?;
            input.parse::<Token![:]>()?;

            let name: Ident = input.parse()?;

            input.parse::<Token![,]>()?;

            name
        };

        let row = if input.peek(row_keyword::row) {
            input.parse::<row_keyword::row>()?;
            input.parse::<Token![:]>()?;

            let content;
            braced![content in input];
            let row: Vec<_> = content
                .parse_terminated(RowField::parse, Token![,])?
                .into_iter()
                .collect();

            input.parse::<Token![,]>()?;

            Some(row)
        } else {
            None
        };

        let optional_params = if input.peek(optional_params_keyword::optional_params) {
            input.parse::<optional_params_keyword::optional_params>()?;
            input.parse::<Token![:]>()?;

            let content;
            bracketed![content in input];
            let optional_params: Vec<_> = content
                .parse_terminated(LitInt::parse, Token![,])?
                .iter()
                .map(|v| v.base10_parse().unwrap())
                .collect();

            input.parse::<Token![,]>()?;

            Some(optional_params)
        } else {
            None
        };

        let query = {
            input.parse::<query_keyword::query>()?;
            input.parse::<Token![:]>()?;

            input.parse()?
        };

        Ok(Self {
            name,
            row,
            optional_params,
            query,
        })
    }
}

mod name_keyword {
    syn::custom_keyword!(name);
}
mod optional_params_keyword {
    syn::custom_keyword!(optional_params);
}
mod row_keyword {
    syn::custom_keyword!(row);
}
mod query_keyword {
    syn::custom_keyword!(query);
}
