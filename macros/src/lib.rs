extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Fields, ItemFn, ItemStruct};

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
            #module_path::simulation::id::reset_store();
            #fn_body
        }
    };

    TokenStream::from(expanded)
}

// Automatically implement EventTrait for any struct with the right fields.
// inspired by https://cetra3.github.io/blog/creating-your-own-derive-macro/
#[proc_macro_derive(EventTrait)]
pub fn event_trait_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);

    // make sure input is a struct
    if let syn::Data::Struct(ref data) = input.data {
        return match data.fields {
            // make sure input is a struct with named fields
            Fields::Named(ref _fields) => {
                let name = input.ident;

                TokenStream::from(quote!(
                        impl crate::simulation::events::EventTrait for #name {
                            fn type_(&self) -> &'static str {
                                Self::TYPE
                            }
                            fn time(&self) -> u32 {
                                self.time
                            }
                            fn attributes(&self) -> &InternalAttributes {
                                &self.attributes
                            }

                }))
            }
            _ => {
                // if it's not a struct with named fields, we can't derive EventTrait
                TokenStream::from(
                    syn::Error::new(
                        input.ident.span(),
                        "Only structs with named fields can derive `EventTrait`",
                    )
                    .to_compile_error(),
                )
            }
        };
    }

    // Catchall if we don't match on the structure we don't want
    TokenStream::from(
        syn::Error::new(
            input.ident.span(),
            "Only structs with named fields can derive `EventTrait`",
        )
        .to_compile_error(),
    )
}

#[proc_macro_attribute]
pub fn event_struct(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::Item);

    let item_struct: ItemStruct = match input {
        syn::Item::Struct(s) => s,
        other => {
            return syn::Error::new_spanned(other, "`#[event_struct]` can only be used on structs")
                .to_compile_error()
                .into();
        }
    };

    let attrs = item_struct.attrs;
    let vis = item_struct.vis;
    let ident = item_struct.ident;
    let generics = item_struct.generics;
    let fields = item_struct.fields;
    let semi = item_struct.semi_token;

    TokenStream::from(quote! {
        #[derive(derive_builder::Builder, Debug, PartialEq, Clone, macros::EventTrait)]
        #(#attrs)*
        #vis struct #ident #generics #fields #semi
    })
}
