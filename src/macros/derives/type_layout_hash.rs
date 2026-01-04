use metrohash::MetroHash64;
use proc_macro::TokenStream;
use quote::quote;
use std::hash::{Hash, Hasher};
use syn::{parse_macro_input, Data, DeriveInput};

pub fn type_layout_hash(input: TokenStream) -> TokenStream
{
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // TODO: this should recurse down into types

    // u128?
    let mut hasher = MetroHash64::with_seed(0);
    match input.data
    {
        Data::Struct(s) => s.fields.hash(&mut hasher),
        Data::Enum(e) => e.variants.hash(&mut hasher),
        Data::Union(u) => u.fields.hash(&mut hasher),
    }
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