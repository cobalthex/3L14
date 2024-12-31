use metrohash::MetroHash64;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::hash::{Hash, Hasher};
use proc_macro2::Ident;
use syn::{parse_macro_input, parse_quote, DeriveInput};

pub fn asset_derive(input: TokenStream) -> TokenStream
{
    let input = parse_macro_input!(input as DeriveInput);
    let ident = &input.ident;
    let handle_ident = format_ident!("{}Handle", ident);


    let z: TokenStream = (quote!
    {
        impl Asset for #ident
        {
            fn asset_type() -> AssetTypeId { AssetTypeId::#ident }
            fn all_dependencies_loaded(&self) -> bool
            {
                true // todo scan struct for any handle refs
            }
        }
        pub type #handle_ident = AssetHandle<#ident>;
    }).into();

    z
}