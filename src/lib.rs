//! Procedural macros for invariant verification.
//!
//! Provides `#[derive(Constitution)]` macro that generates:
//! 1. **Runtime checks** via `CheckProtocol::enforce_law()` - panics on violation
//! 2. **Formal verification** via `FormallyVerifiable::verify_integrity()` - SMT-based proof
//!
//! # Example
//!
//! ```ignore
//! use praborrow_core::CheckProtocol;
//! use praborrow_prover::{FormallyVerifiable, VerificationToken, ProofError};
//!
//! #[derive(Constitution)]
//! struct BoundedValue {
//!     #[invariant("self.value > 0")]
//!     value: i32,
//! }
//!
//! let v = BoundedValue { value: 10 };
//!
//! // Runtime check (panics if violated)
//! v.enforce_law();
//!
//! // Formal verification (returns Result)
//! let token: Result<VerificationToken, ProofError> = v.verify_integrity();
//! ```
//!
//! # Generated Code
//!
//! For each struct with `#[derive(Constitution)]`, the macro generates:
//!
//! - `impl CheckProtocol` with `enforce_law()` - runtime panic checks
//! - `impl FormallyVerifiable` with:
//!   - `INVARIANTS: &'static [&'static str]` - the invariant expressions
//!   - `verify_integrity()` - SMT-based proof returning `Result<VerificationToken, ProofError>`
//!   - `field_values()` - returns field name/value pairs for SMT solver

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, Meta, Type, parse_macro_input};

/// Information about a field with invariants.
#[allow(dead_code)] // Reserved for future Z3 backend integration
struct FieldInfo {
    name: Ident,
    ty: Type,
    invariants: Vec<String>,
}

/// Checks if a type is a supported integer type.
fn is_integer_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        #[allow(clippy::collapsible_if)]
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();
            return matches!(
                ident.as_str(),
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
            );
        }
    }
    false
}

/// Derives the Constitution trait for a struct.
///
/// Generates both runtime (panic-based) and formal (SMT-based) verification.
#[proc_macro_derive(Constitution, attributes(invariant))]
pub fn derive_constitution(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut runtime_checks = Vec::new();
    let mut invariant_strings = Vec::new();
    let mut field_infos: Vec<FieldInfo> = Vec::new();
    let mut all_fields: Vec<(Ident, Type)> = Vec::new();

    if let Data::Struct(syn::DataStruct {
        fields: Fields::Named(fields),
        ..
    }) = &input.data
    {
        for field in &fields.named {
            let field_name = field.ident.clone().expect("Named field must have ident");
            let field_type = field.ty.clone();
            all_fields.push((field_name.clone(), field_type.clone()));

            let mut field_invariants = Vec::new();

            for attr in &field.attrs {
                if let Meta::List(meta_list) = &attr.meta {
                    #[allow(clippy::collapsible_if)]
                    if meta_list.path.is_ident("invariant") {
                        // Parse the invariant condition expression directly
                        // Parse the invariant condition expression directly
                        match meta_list.parse_args::<syn::Expr>() {
                            Ok(expr) => {
                                // Extract the invariant string and tokens
                                let (condition_str, condition_tokens) =
                                    if let syn::Expr::Lit(syn::ExprLit {
                                        lit: syn::Lit::Str(lit_str),
                                        ..
                                    }) = &expr
                                    {
                                        let s = lit_str.value();
                                        // For string literals, we must parse the content to get tokens for runtime check
                                        match syn::parse_str::<syn::Expr>(&s) {
                                            Ok(e) => (s, quote! { #e }),
                                            Err(err) => {
                                                return syn::Error::new_spanned(
                                                    lit_str,
                                                    format!(
                                                        "Syntax error in invariant string: {}",
                                                        err
                                                    ),
                                                )
                                                .to_compile_error()
                                                .into();
                                            }
                                        }
                                    } else {
                                        let tokens = quote! { #expr };
                                        (tokens.to_string(), tokens)
                                    };

                                // Validate invariant syntax at compile time using Prover Parser
                                if let Err(e) = praborrow_prover::parser::ExpressionParser::parse(
                                    &condition_str,
                                ) {
                                    let err_msg = format!("Invalid invariant syntax: {}", e);
                                    return syn::Error::new_spanned(&expr, err_msg)
                                        .to_compile_error()
                                        .into();
                                }

                                field_invariants.push(condition_str.clone());
                                invariant_strings.push(condition_str.clone());

                                // Correctly construct the new ConstitutionError structure
                                runtime_checks.push(quote! {
                                        if !(#condition_tokens) {
                                            return Err(praborrow_core::ConstitutionError::InvariantViolation {
                                                expression: #condition_str.to_string(),
                                                values: std::collections::BTreeMap::new(),
                                            });
                                        }
                                    });
                            }
                            Err(e) => {
                                return TokenStream::from(e.to_compile_error());
                            }
                        }
                    }
                }
            }

            if !field_invariants.is_empty() {
                field_infos.push(FieldInfo {
                    name: field_name,
                    ty: field_type,
                    invariants: field_invariants,
                });
            }
        }
    }

    // Generate the invariant strings as a static array
    let invariant_count = invariant_strings.len();
    let invariant_literals: Vec<_> = invariant_strings
        .iter()
        .map(|s| syn::LitStr::new(s, proc_macro2::Span::call_site()))
        .collect();

    // Generate field value extraction for hash computation
    // Only include integer fields for now
    let hash_fields: Vec<_> = all_fields
        .iter()
        .filter(|(_, ty)| is_integer_type(ty))
        .map(|(name, _)| {
            quote! {
                hasher.update(&self.#name.to_le_bytes());
            }
        })
        .collect();

    // Generate field provider implementation
    // Maps field names to Z3 AST values
    let field_match_arms: Vec<_> = all_fields
        .iter()
        .filter(|(_, ty)| is_integer_type(ty))
        .map(|(name, ty)| {
            let name_str = name.to_string();
            let is_unsigned = if let Type::Path(tp) = ty {
                tp.path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string().starts_with('u'))
                    .unwrap_or(false)
            } else {
                false
            };

            if is_unsigned {
                quote! {
                    #name_str => {
                        Ok(FieldValue::UInt(self.0.#name as u64))
                    }
                }
            } else {
                quote! {
                    #name_str => {
                        Ok(FieldValue::Int(self.0.#name as i64))
                    }
                }
            }
        })
        .collect();

    let expanded = quote! {
        // Runtime check implementation - returns Result instead of panicking
        impl CheckProtocol for #name {
            fn enforce_law(&self) -> Result<(), praborrow_core::ConstitutionError> {
                #(#runtime_checks)*
                Ok(())
            }
        }

        // Formal verification implementation
        impl praborrow_prover::ProveInvariant for #name {
            fn invariant_expressions() -> &'static [&'static str] {
                static INVARIANTS: [&str; #invariant_count] = [#(#invariant_literals),*];
                &INVARIANTS
            }

            fn compute_data_hash(&self) -> Vec<u8> {
                use praborrow_prover::sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                #(#hash_fields)*
                hasher.finalize().to_vec()
            }

            fn get_field_provider(&self) -> alloc::boxed::Box<dyn praborrow_prover::backend::FieldValueProvider + '_> {
                 use praborrow_prover::backend::{FieldValueProvider, FieldValue};
                 use praborrow_prover::ProofError;

                 struct FieldProvider<'a>(&'a #name);

                 impl<'a> FieldValueProvider for FieldProvider<'a> {
                    fn get_field_value(&self, name: &str) -> Result<FieldValue, ProofError> {
                        match name {
                            #(#field_match_arms)*
                            _ => Err(ProofError::ParseError(format!("Unknown field: {}", name))),
                        }
                    }
                 }

                 alloc::boxed::Box::new(FieldProvider(self))
            }

            fn verify_with_context(
                &self,
                ctx: &praborrow_prover::SmtContext
            ) -> impl core::future::Future<Output = Result<praborrow_prover::VerificationToken, praborrow_prover::ProofError>> + Send {
                async move {
                    let provider = self.get_field_provider();
                    ctx.verify_invariants(&*provider, Self::invariant_expressions()).await
                }
            }
        }
    };

    TokenStream::from(expanded)
}
