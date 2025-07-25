use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

pub fn create_main_struct(name: &Ident, query: &str, parameters: &[syn::Type]) -> TokenStream {
    let parameter_fields: Vec<_> = parameters
        .iter()
        .enumerate()
        .map(|(index, field_type)| {
            let name = format_ident!("p{}", index + 1);
            quote! {
                #name: #field_type
            }
        })
        .collect();

    let parameter_names: Vec<_> = parameters
        .iter()
        .enumerate()
        .map(|(index, _)| format_ident!("p{}", index + 1))
        .collect();

    let self_parameter_names = parameter_names.iter().map(|param| quote!(&self.#param));

    let parameter_count = parameters.len();

    quote! {
        struct #name<'a> {
            #( #parameter_fields , )*
            pub phantom_data: core::marker::PhantomData<&'a ()>,
        }
        impl<'a> #name<'a> {
            pub const QUERY: &'static str = #query;
            pub fn params(#( #parameter_fields ),*) -> Self {
                Self {
                    #( #parameter_names , )*
                    phantom_data: core::marker::PhantomData,
                }
            }

            pub fn as_array(&'a self) -> [&'a (dyn ts_sql_helper_lib::postgres::types::ToSql + Sync); #parameter_count] {
                [
                    #( #self_parameter_names , )*
                ]
            }
        }
    }
}
