use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::query::RowField;

pub fn create_row_struct(query_struct_name: &Ident, fields: &[RowField]) -> TokenStream {
    let name = format_ident!("{query_struct_name}Row");

    quote! {
        #[derive(ts_sql_helper_lib::FromRow)]
        struct #name {
            #( #fields , )*
        }
    }
}
