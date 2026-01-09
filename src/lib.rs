use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Meta, Lit};

#[proc_macro_derive(Constitution, attributes(invariant))]
pub fn derive_constitution(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut checks = Vec::new();

    if let Data::Struct(data) = input.data {
        if let Fields::Named(fields) = data.fields {
            for field in fields.named {
                for attr in field.attrs {
                    if let Meta::List(meta_list) = attr.meta {
                        if meta_list.path.is_ident("invariant") {
                            // Parse the invariant condition string
                            match meta_list.parse_args::<Lit>() {
                                Ok(Lit::Str(lit_str)) => {
                                    let condition_str = lit_str.value();
                                    
                                    // Parse string into TokenStream to support expressions
                                    let condition_tokens: proc_macro2::TokenStream = condition_str.parse().expect("Invalid invariant condition syntax");
                                    
                                    let error_msg = format!("CONSTITUTIONAL CRISIS: Invariant '{}' breached.", condition_str);
                                    let error_msg_lit = syn::LitStr::new(&error_msg, proc_macro2::Span::call_site());
                                    
                                    checks.push(quote! {
                                        if !(#condition_tokens) {
                                            panic!(#error_msg_lit);
                                        }
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    // We assume CheckProtocol is in scope or available via praborrow::core::CheckProtocol
    // BUT since this is a separate crate, and we don't know if the user imported it,
    // we should try to use a qualified path if possible, or expect the user to have it.
    // Given the structure, `praborrow::core::CheckProtocol` is the safe bet, assuming `praborrow` crate is present.
    // However, inside the workspace, `praborrow` might not be the name if using direct deps.
    // Let's assume the user imports `CheckProtocol` trait.
    
    let expanded = quote! {
        impl CheckProtocol for #name {
            fn enforce_law(&self) {
                #(#checks)*
            }
        }
    };

    TokenStream::from(expanded)
}
