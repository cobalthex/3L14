use metrohash::MetroHash64;
use proc_macro::TokenStream;
use quote::quote;
use std::hash::{Hash, Hasher};
use proc_macro2::Ident;
use syn::{parse_macro_input, parse_quote, DeriveInput};

pub const TYPE_LAYOUT_HASH_SEED: u64 = 0x1337C0DE00000000; // NOTE: This will change all usages if this changes

pub fn layout_hash(input: TokenStream) -> TokenStream
{
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // u128?
    let mut hasher = MetroHash64::with_seed(TYPE_LAYOUT_HASH_SEED);
    input.hash(&mut hasher); // this may hash more data than necessary, but should be pretty good
    let hash = hasher.finish();

    // trait?
    
    (quote!
    {
        impl #name
        {
            pub const TYPE_LAYOUT_HASH: u64 = #hash;
        }
    }).into()
}