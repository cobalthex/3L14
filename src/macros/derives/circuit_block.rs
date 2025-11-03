use proc_macro::TokenStream;
use proc_macro2::Span;
use std::hash::Hasher;
use metrohash::MetroHash64;
use quote::quote;
use syn::{parse_macro_input, parse_str, DeriveInput, Fields, Ident, Path, Type};
use syn::Data::Struct;

fn path_contains(container: &Path, containing: &Path) -> bool
{
    if container.segments.len() < containing.segments.len()
    {
        return false;
    }

    let skip = if containing.leading_colon.is_some() { 0 }
                      else { container.segments.len() - containing.segments.len() };

    container.segments.iter()
        .skip(skip)
        .zip(containing.segments.iter())
        .all(|(seg, t)| seg.ident == t.ident)
}

#[derive(Debug)]
enum IsOutlet
{
    No,
    Pulsed,
    Latching,
}
fn is_outlet(ty: &Type, test_pulsed: &Path, test_latching: &Path) -> IsOutlet
{
    match ty
    {
        Type::Array(syn::TypeArray { elem, .. }) => is_outlet(&elem, test_pulsed, test_latching),
        Type::Path(path) =>
            {
                if path_contains(test_pulsed, &path.path) { IsOutlet::Pulsed }
                else if path_contains(test_latching, &path.path) { IsOutlet::Latching }
                else { IsOutlet::No }
            },
        Type::Slice(syn::TypeSlice { elem, .. }) => is_outlet(&elem, test_pulsed, test_latching),
        _ => IsOutlet::No,
    }
}

pub fn circuit_block(input: TokenStream) -> TokenStream
{
    let input = parse_macro_input!(input as DeriveInput);
    let Struct(data) = &input.data else { panic!("Circuit blocks must be structs") };

    let typename_ident = &input.ident;

    let (latch_crate_name, type_name_hash) =
    {
        let mut name = std::env::var("CARGO_PKG_NAME").unwrap();
        let name_hash =
        {
            let mut hasher = MetroHash64::with_seed(0);
            hasher.write(name.as_bytes());
            hasher.write(name.to_string().as_bytes());
            hasher.finish()
        };

        (if name == "latch_3l14" { "crate" } else { "latch_3l14" }, name_hash)
    };

    let path = |mod_path: &str| -> Path { parse_str(&format!("{latch_crate_name}::{mod_path}")).unwrap() };

    let path_pulsed = path("PulsedOutlet");
    let path_latching = path("LatchingOutlet");

    let fields: Box<[_]> = match &data.fields
    {
        Fields::Named(named) =>
        {
            named.named.iter().map(|field| (field.ident.clone().unwrap(), is_outlet(&field.ty, &path_pulsed, &path_latching)) )
                .collect()
        },
        Fields::Unnamed(unnamed) =>
        {
            todo!()
        }
        Fields::Unit => { Box::new([]) }
    };

    let hydrate_fn_lines = fields.iter().map(|(ident, is_outlet)|
    {
        let fname = ident.to_string();
        match is_outlet
        {
            IsOutlet::No => quote!
            {
                // TODO: error handling
                #ident:
                {
                    if let Some(mut field) = hydration.fields.remove(&unicase::UniCase::unicode(#fname))
                    {
                        ::erased_serde::deserialize(&mut field)?
                    }
                    else { Default::default() }
                }
            },
            IsOutlet::Pulsed => quote! { #ident: hydration.pulsed_outlets.remove(&unicase::UniCase::unicode(#fname)).unwrap_or_default() },
            IsOutlet::Latching => quote! { #ident: hydration.latching_outlets.remove(&unicase::UniCase::unicode(#fname)).unwrap_or_default() },
        }
    });

    // TODO: iter_all_outlets

    let typename_str = typename_ident.to_string();

    let meta_ident = Ident::new(&format!("{}_BLOCK_META", typename_str.to_uppercase()), Span::call_site());
    let path_hydrate = path("block_meta::HydrateBlock");
    let path_block = path("Block");
    let path_blockmeta = path("block_meta::BlockMeta");
    let path_kind = path("blocks::BlockKind");
    let path_bothimpls = path("block_meta::CannotImplBothBlockTypes");

    quote!
    {
        const #meta_ident: #path_blockmeta = #path_blockmeta
        {
            type_name: #typename_str,
            type_name_hash: #type_name_hash,
            hydrate_fn: |hydration: &mut #path_hydrate|
            {
                Ok(Box::new(#typename_ident
                {
                    #(#hydrate_fn_lines),*
                }))
            },
            kind: #path_kind::Impulse, // TODO
        };
        impl #path_block for #typename_ident
        {
            fn meta(&self) -> &'static #path_blockmeta { &#meta_ident }
        }
        impl #path_bothimpls for #typename_ident { }
        ::inventory::submit! { #meta_ident }
    }.into()
}


// TODO: tests for path_contains
// same paths
// left longer
// right longer
// subpaths
// test-path starts with :: and tests from root