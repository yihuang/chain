use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, parse_quote, DeriveInput, Ident};

#[proc_macro_derive(CryptoHasher)]
pub fn hasher_dispatch(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let hasher_name = Ident::new(
        &format!("{}Hasher", &ast.ident.to_string()),
        Span::call_site(),
    );
    let snake_name = camel_to_snake(&ast.ident.to_string());
    let static_hasher_name = Ident::new(
        &format!("{}_HASHER", snake_name.to_uppercase()),
        Span::call_site(),
    );
    let type_name = &ast.ident;
    let generics = add_trait_bounds(ast.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let out = quote!(
        #[derive(Clone)]
        pub struct #hasher_name(chain_crypto::hasher::DefaultHasher);

        static #static_hasher_name: chain_crypto::_once_cell::sync::Lazy<#hasher_name> =
            chain_crypto::_once_cell::sync::Lazy::new(|| #hasher_name::new());

        impl #hasher_name {
            fn new() -> Self {
                let name = stringify!(#type_name);
                #hasher_name(
                    chain_crypto::hasher::DefaultHasher::new(&name.as_bytes()))
            }
        }

        impl std::default::Default for #hasher_name {
            fn default() -> Self {
                #static_hasher_name.clone()
            }
        }

        impl chain_crypto::hasher::CryptoHasher for #hasher_name {
            fn update(&mut self, bytes: &[u8]) {
                self.0.update(bytes);
            }

            fn finish(self) -> chain_crypto::hash_value::HashValue {
                self.0.finish()
            }
        }

        impl #impl_generics chain_crypto::hasher::HasCryptoHasher for #type_name #ty_generics #where_clause {
            type Hasher = #hasher_name;
        }
    );
    out.into()
}

#[proc_macro_derive(PSCCryptoHash)]
pub fn psc_crypto_hash_dispatch(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let hasher_name = Ident::new(
        &format!("{}Hasher", &ast.ident.to_string()),
        Span::call_site(),
    );
    let name = &ast.ident;
    let generics = add_trait_bounds(ast.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let out = quote!(
        impl #impl_generics chain_crypto::hasher::CryptoHash for #name #ty_generics #where_clause {
            fn hash(&self) -> chain_crypto::hash_value::HashValue {
                use chain_crypto::hasher::CryptoHasher;
                let mut state = #hasher_name::default();
                state.update(&self.encode());
                state.finish()
            }
        }
    );
    out.into()
}

#[proc_macro_derive(AsRefCryptoHash)]
pub fn as_ref_crypto_hash_dispatch(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let hasher_name = Ident::new(
        &format!("{}Hasher", &ast.ident.to_string()),
        Span::call_site(),
    );
    let name = &ast.ident;
    let generics = add_trait_bounds(ast.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let out = quote!(
        impl #impl_generics chain_crypto::hasher::CryptoHash for #name #ty_generics #where_clause {
            fn hash(&self) -> chain_crypto::hash_value::HashValue {
                use chain_crypto::hasher::CryptoHasher;

                let mut state = #hasher_name::default();
                state.update(self.as_ref());
                state.finish()
            }
        }
    );
    out.into()
}

fn add_trait_bounds(mut generics: syn::Generics) -> syn::Generics {
    for param in generics.params.iter_mut() {
        if let syn::GenericParam::Type(type_param) = param {
            type_param.bounds.push(parse_quote!(Serialize));
        }
    }
    generics
}

/// Converts a camel-case string to snake-case
fn camel_to_snake(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut first = true;
    text.chars().for_each(|c| {
        if !first && c.is_uppercase() {
            out.push('_');
            out.extend(c.to_lowercase());
        } else if first {
            first = false;
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    });
    out
}
