use metrohash::MetroHash64;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::hash::{Hash, Hasher};
use proc_macro2::Ident;
use syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, Type};

pub fn asset_derive(input: TokenStream) -> TokenStream
{
    let input = parse_macro_input!(input as DeriveInput);
    let ident = &input.ident;
    // let handle_ident = format_ident!("{}Handle", ident);

    let mut asset_handles = Vec::new();
    if let Data::Struct(s) = input.data
    {
        let mut i = 0usize;
        for f in s.fields
        {
            match f.ty
            {
                // Type::Array(_) => {}
                Type::Path(path) =>
                {
                    if path.path.segments.iter().any(|seg| seg.ident == "AssetHandle")
                    {
                        asset_handles.push(f.ident.unwrap_or_else(|| format_ident!("{}", i)));
                    }
                },
                // Type::Ptr(_) => {}
                // Type::Reference(_) => {}
                // Type::Slice(_) => {}
                // Type::Tuple(_) => {}
                _ => {},
            }
            i += 1;
        }
    }

    let mut handle_refs = quote!{ #(self.#asset_handles.all_dependencies_loaded())&&* };
    if handle_refs.is_empty() { handle_refs = quote!(true) };

    let z: TokenStream = (quote!
    {
        impl Asset for #ident
        {
            fn asset_type() -> AssetTypeId { AssetTypeId::#ident }
            fn all_dependencies_loaded(&self) -> bool
            {
                #handle_refs
            }
        }
        // pub type #handle_ident = AssetHandle<#ident>;
    }).into();

    z
}