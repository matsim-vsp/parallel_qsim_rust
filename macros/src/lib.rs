extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn integration_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let module_path: syn::Path = if attr.is_empty() {
        syn::parse_quote!(crate)
    } else {
        parse_macro_input!(attr as syn::Path)
    };
    let fn_name = &input_fn.sig.ident;
    let fn_body = &input_fn.block;
    let attrs = &input_fn.attrs;
    let fn_return_type = &input_fn.sig.output;

    let expanded = quote! {
        #(#attrs)*
        #[test]
        #[serial_test::serial]
        fn #fn_name() #fn_return_type {
            #module_path::simulation::id::init_store();
            #module_path::simulation::id::reset_store();
            #fn_body
        }
    };

    TokenStream::from(expanded)
}
