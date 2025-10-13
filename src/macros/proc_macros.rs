use proc_macro::TokenStream;
use derives::{fancy_enum, flags_enum, type_layout_hash};
use attribs::asset;
use crate::derives::circuit_block_meta;

mod derives;
mod has_derive;
mod attribs;
mod case_conv;

// FancyEnum adds .variant_ident() and for each #[enum_prop(k=v)] a method k() returning v
#[proc_macro_derive(FancyEnum, attributes(enum_prop))]
pub fn fancy_enum_derive(input: TokenStream) -> TokenStream
{
    fancy_enum::fancy_enum(input)
}

// Adds standard bit ops and .has_flag(..) to enums
#[proc_macro_derive(Flags)]
pub fn flags_enum_derive(input: TokenStream) -> TokenStream
{
    flags_enum::flags_enum(input)
}

#[proc_macro_derive(LayoutHash)]
pub fn type_layout_hash_derive(input: TokenStream) -> TokenStream { type_layout_hash::type_layout_hash(input) }

#[proc_macro_derive(CircuitBlock)]
pub fn circuit_block_meta(input: TokenStream) -> TokenStream { circuit_block_meta::derive_circuit_block_meta(input) }

#[proc_macro_attribute] // todo: better name?
pub fn asset(attrib_input: TokenStream, input: TokenStream) -> TokenStream { asset::asset_attrib(attrib_input, input) }

#[proc_macro]
pub fn pascal_to_title(input: TokenStream) -> TokenStream { case_conv::pascal_to_title(input) }