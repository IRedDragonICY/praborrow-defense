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
use syn::{parse_macro_input, DeriveInput, Data, Fields, Meta, Type, Ident};

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
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();
            return matches!(
                ident.as_str(),
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" |
                "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
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

    if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            for field in &fields.named {
                let field_name = field.ident.clone().expect("Named field must have ident");
                let field_type = field.ty.clone();
                all_fields.push((field_name.clone(), field_type.clone()));
                
                let mut field_invariants = Vec::new();
                
                for attr in &field.attrs {
                    if let Meta::List(meta_list) = &attr.meta {
                        if meta_list.path.is_ident("invariant") {
                            // Parse the invariant condition expression directly
                            match meta_list.parse_args::<syn::Expr>() {
                                Ok(expr) => {
                                    let condition_tokens = quote! { #expr };
                                    let condition_str = condition_tokens.to_string();
                                    
                                    field_invariants.push(condition_str.clone());
                                    invariant_strings.push(condition_str.clone());
                                    
                                    let error_msg = format!(
                                        "CONSTITUTIONAL CRISIS: Invariant '{}' breached.",
                                        condition_str
                                    );
                                    let error_msg_lit = syn::LitStr::new(
                                        &error_msg,
                                        proc_macro2::Span::call_site()
                                    );
                                    
                                    runtime_checks.push(quote! {
                                        if !(#condition_tokens) {
                                            return Err(praborrow_core::ConstitutionError::InvariantViolation(#error_msg_lit.to_string()));
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
    }

    // Generate the invariant strings as a static array
    let invariant_count = invariant_strings.len();
    let invariant_literals: Vec<_> = invariant_strings.iter()
        .map(|s| syn::LitStr::new(s, proc_macro2::Span::call_site()))
        .collect();

    // Generate field value extraction for hash computation
    // Only include integer fields for now
    let hash_fields: Vec<_> = all_fields.iter()
        .filter(|(_, ty)| is_integer_type(ty))
        .map(|(name, _)| {
            quote! {
                hasher.update(&self.#name.to_le_bytes());
            }
        })
        .collect();

    // Generate field provider implementation
    // Maps field names to Z3 AST values
    let field_match_arms: Vec<_> = all_fields.iter()
        .filter(|(_, ty)| is_integer_type(ty))
        .map(|(name, ty)| {
            let name_str = name.to_string();
            let is_unsigned = if let Type::Path(tp) = ty {
                 tp.path.segments.last().map(|s| s.ident.to_string().starts_with('u')).unwrap_or(false)
            } else { false };
            
            if is_unsigned {
                 quote! {
                    #name_str => {
                        Ok(ast::Int::from_u64(ctx, self.0.#name as u64))
                    }
                }
            } else {
                 quote! {
                    #name_str => {
                        Ok(ast::Int::from_i64(ctx, self.0.#name as i64))
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
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                #(#hash_fields)*
                hasher.finalize().to_vec()
            }

            fn verify_with_context(
                &self,
                ctx: &praborrow_prover::SmtContext
            ) -> Result<praborrow_prover::VerificationToken, praborrow_prover::ProofError> {
                use praborrow_prover::parser::FieldValueProvider;
                use praborrow_prover::{Context, ast};

                // Create a field provider for this instance
                struct FieldProvider<'a>(&'a #name);
                
                impl<'a, 'ctx> FieldValueProvider<'ctx> for FieldProvider<'a> {
                    fn get_field_z3(
                        &self,
                        ctx: &'ctx Context,
                        field_name: &str
                    ) -> Result<ast::Int<'ctx>, praborrow_prover::ProofError> {
                        match field_name {
                            #(#field_match_arms)*
                            _ => Err(praborrow_prover::ProofError::ParseError(
                                format!("Unknown field: {}", field_name)
                            ))
                        }
                    }
                }

                let provider = FieldProvider(self);
                ctx.verify_invariants(&provider, Self::invariant_expressions())
            }
        }
    };

    TokenStream::from(expanded)
}
