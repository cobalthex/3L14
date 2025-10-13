use proc_macro::TokenStream;
use std::hash::Hasher;
use metrohash::MetroHash64;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_circuit_block_meta(input: TokenStream) -> TokenStream
{
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let mut crate_name = std::env::var("CARGO_PKG_NAME").unwrap();
    // if crate_name == "latch_3l14" { crate_name = "crate".to_string(); } // blocks defined in the origin crate

    let name_hash =
    {
        let mut hasher = MetroHash64::with_seed(0);
        hasher.write(crate_name.as_bytes());
        hasher.write(name.to_string().as_bytes());
        hasher.finish()
    };

    quote!
    {
        impl #crate_name::block_meta::BlockMeta for #name
        {
            const TYPE_NAME: &'static str = #name;
            const BLOCK_NAME_HASH: u64 = #name_hash;
        }
        ::inventory::submit! {
            #crate_name::block_meta::BlockRegistration::register::<#name>()
        }
    }.into()
}
