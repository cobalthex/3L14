use proc_macro::TokenStream;
use proc_macro2::Span;
use std::hash::Hasher;
use metrohash::MetroHash64;
use quote::quote;
use syn::{parse_macro_input, parse_str, DeriveInput, Fields, Ident, Member, Path, Type};
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
        let name = std::env::var("CARGO_PKG_NAME").unwrap();
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
    let path_impls = path("impls::impls");
    let path_default_if = path("block_meta::default_if");

    let fields: Box<[_]> = match &data.fields
    {
        Fields::Named(named) =>
        {
            named.named.iter().map(|field|
                (Member::Named(field.ident.clone().unwrap()), &field.ty, is_outlet(&field.ty, &path_pulsed, &path_latching)) )
                .collect()
        },
        Fields::Unnamed(unnamed) =>
        {
            unnamed.unnamed.iter().enumerate().map(|(i, field)|
                (Member::Unnamed(i.into()), &field.ty, is_outlet(&field.ty, &path_pulsed, &path_latching)) )
                .collect()
        }
        Fields::Unit => { Box::new([]) }
    };

    let hydrate_fn_lines = fields.iter().map(|(field, fty, is_outlet)|
    {
        let fname = match field
        {
            Member::Named(n) => n.to_string(),
            Member::Unnamed(i) => i.index.to_string(),
        };
        match is_outlet
        {
            IsOutlet::No => quote!
            {
                #field: match hydration.fields.remove(&unicase::UniCase::unicode(#fname))
                {
                    Some(mut v) => ::erased_serde::deserialize(&mut v)?,
                    None =>
                    {
                         const CAN_DEFAULT: bool = #path_impls!(#fty: Default);
                        #path_default_if::<_, CAN_DEFAULT>()
                    }
                }
            },
            IsOutlet::Pulsed => quote!
            {
                #field: hydration.pulsed_outlets.remove(&unicase::UniCase::unicode(#fname)).unwrap_or_default()
            },
            IsOutlet::Latching => quote!
            {
                #field: hydration.latching_outlets.remove(&unicase::UniCase::unicode(#fname)).unwrap_or_default()
            },
        }
    });

    // TODO: iter_all_outlets

    let typename_str = typename_ident.to_string();

    let path_hydrate = path("block_meta::HydrateBlock");
    let path_block = path("Block");
    let path_latchblock = path("LatchBlock");
    let path_impulseblock = path("ImpulseBlock");
    let path_blockmeta = path("block_meta::BlockMeta");

    let ts = quote!
    {
        impl #path_block for #typename_ident
        {
        }
        ::inventory::submit!
        {
            const BLOCK_KIND_VAL: u8 = #path_impls!(#typename_ident: #path_latchblock) as u8;
            #path_blockmeta::<BLOCK_KIND_VAL>
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
            }
        }
    }.into();
    ts
}


// TODO: tests for path_contains
// same paths
// left longer
// right longer
// subpaths
// test-path starts with :: and tests from root
