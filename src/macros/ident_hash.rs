use proc_macro::TokenStream;
use std::hash::{Hash, Hasher};
use metrohash::MetroHash64;
use quote::quote;

pub fn ident_hash32(input: TokenStream) -> TokenStream
{
    let ident = syn::parse::<syn::Ident>(input).expect("Argument must be an identifier");
    let mut hasher = MetroHash64::with_seed(0);
    ident.hash(&mut hasher);
    let hash = hasher.finish() as u32;
    (quote! { #hash }).into()
}