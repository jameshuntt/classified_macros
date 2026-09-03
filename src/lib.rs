//! `#[derive(Classified)]` for the [`classified`](https://crates.io/crates/classified) crate.
//!
//! Use it through `classified` with the `derive` feature; this crate is the
//! implementation and has no API of its own beyond the derive.
//!
//! Applied to a struct with named fields, the derive generates:
//!
//! * a `Drop` impl that zeroizes every field, so each field type must
//!   implement `zeroize::Zeroize`;
//! * a `Debug` impl that prints every field as `[REDACTED]`;
//! * a `<Name>View<'view>` struct with a `pub` reference to each field;
//! * an inherent `expose(&self, op)` that lends that view to a closure, and
//!   an impl of `classified::Expose` doing the same.
//!
//! A field marked `#[classified(public)]` is not a secret: it is printed as
//! usual, left alone on drop, and still present in the view.
//!
//! ```ignore
//! use classified::Classified;
//!
//! #[derive(Classified)]
//! pub struct Wallet {
//!     seed: [u8; 32],
//!     passphrase: String,
//!     #[classified(public)]
//!     label: String,
//! }
//! ```
//!
//! Generic structs are supported; put the `Zeroize` bound on the struct's own
//! generics, since a `Drop` impl cannot add bounds the struct does not have.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Error, Fields, GenericParam, Ident, Lifetime, LifetimeParam,
    Result, Type, WhereClause,
};

/// See the crate documentation.
#[proc_macro_derive(Classified, attributes(classified))]
pub fn derive_classified(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input).unwrap_or_else(Error::into_compile_error).into()
}

struct Field {
    ident: Ident,
    ty: Type,
    public: bool,
}

fn expand(input: DeriveInput) -> Result<TokenStream2> {
    let name = &input.ident;
    let vis = &input.vis;
    let named = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => return Err(Error::new_spanned(name, "`Classified` needs a struct with named fields")),
        },
        _ => return Err(Error::new_spanned(name, "`Classified` can only be derived for a struct")),
    };

    let mut fields = Vec::new();
    for field in named {
        let mut public = false;
        for attr in &field.attrs {
            if attr.path().is_ident("classified") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("public") {
                        public = true;
                        Ok(())
                    } else {
                        Err(meta.error("expected `#[classified(public)]`"))
                    }
                })?;
            }
        }
        fields.push(Field { ident: field.ident.clone().expect("named field"), ty: field.ty.clone(), public });
    }
    if fields.is_empty() {
        return Err(Error::new_spanned(name, "`Classified` needs at least one field"));
    }

    let name_str = name.to_string();
    let view_name = format_ident!("{name}View");
    let view_lt = Lifetime::new("'view", Span::call_site());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut view_generics = input.generics.clone();
    view_generics.params.insert(0, GenericParam::Lifetime(LifetimeParam::new(view_lt.clone())));
    let (view_impl_generics, view_ty_generics, view_where) = view_generics.split_for_impl();

    // Debug may ask more of the public field types than the struct does.
    let mut debug_where: WhereClause = where_clause.cloned().unwrap_or_else(|| parse_quote!(where));
    for field in fields.iter().filter(|f| f.public) {
        let ty = &field.ty;
        debug_where.predicates.push(parse_quote!(#ty: ::core::fmt::Debug));
    }

    let idents: Vec<&Ident> = fields.iter().map(|f| &f.ident).collect();
    let tys: Vec<&Type> = fields.iter().map(|f| &f.ty).collect();
    let field_docs: Vec<String> = idents.iter().map(|i| format!("A borrow of `{i}`.")).collect();
    let view_doc = format!("A borrow of every field of [`{name_str}`], lent to an `expose` closure.");

    let debug_fields = fields.iter().map(|f| {
        let ident = &f.ident;
        let label = ident.to_string();
        if f.public {
            quote! { .field(#label, &self.#ident) }
        } else {
            quote! { .field(#label, &::core::format_args!("[REDACTED]")) }
        }
    });
    let zeroize_fields = fields.iter().filter(|f| !f.public).map(|f| {
        let ident = &f.ident;
        quote! { ::classified::zeroize::Zeroize::zeroize(&mut self.#ident); }
    });

    Ok(quote! {
        #[doc = #view_doc]
        #vis struct #view_name #view_impl_generics #view_where {
            #( #[doc = #field_docs] pub #idents: &#view_lt #tys, )*
        }

        impl #impl_generics #name #ty_generics #where_clause {
            /// Lend a view of every field to `op` for the duration of the call.
            #vis fn expose<__R>(
                &self,
                op: impl for<#view_lt> FnOnce(#view_name #view_ty_generics) -> __R,
            ) -> __R {
                op(#view_name { #( #idents: &self.#idents, )* })
            }
        }

        impl #impl_generics ::classified::Expose for #name #ty_generics #where_clause {
            type View<#view_lt> = #view_name #view_ty_generics where Self: #view_lt;

            fn expose<'__s, __R>(&'__s self, op: impl FnOnce(Self::View<'__s>) -> __R) -> __R {
                op(#view_name { #( #idents: &self.#idents, )* })
            }
        }

        impl #impl_generics ::core::fmt::Debug for #name #ty_generics #debug_where {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(#name_str) #(#debug_fields)* .finish()
            }
        }

        impl #impl_generics ::core::ops::Drop for #name #ty_generics #where_clause {
            fn drop(&mut self) {
                #(#zeroize_fields)*
            }
        }
    })
}
