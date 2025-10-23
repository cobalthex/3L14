use proc_macro::TokenStream;
use derives::{fancy_enum, flags_enum, type_layout_hash};
use attribs::asset;
use crate::derives::circuit_block;

mod derives;
mod has_derive;
mod attribs;
mod case_conv;

// FancyEnum adds .variant_ident() and for each #[enum_prop(k=v)] a method k() returning v
#[proc_macro_derive(FancyEnum, attributes(enum_prop))]
pub fn derive_fancy_enum(input: TokenStream) -> TokenStream
{
    fancy_enum::fancy_enum(input)
}

// Adds standard bit ops and .has_flag(..) to enums
#[proc_macro_derive(Flags)]
pub fn derive_flags_enum(input: TokenStream) -> TokenStream
{
    flags_enum::flags_enum(input)
}

#[proc_macro_derive(LayoutHash)]
pub fn derive_type_layout_hash(input: TokenStream) -> TokenStream { type_layout_hash::type_layout_hash(input) }

#[proc_macro_derive(CircuitBlock)]
pub fn derive_circuit_block(input: TokenStream) -> TokenStream { circuit_block::circuit_block(input) }

#[proc_macro_attribute] // todo: better name?
pub fn asset(attrib_input: TokenStream, input: TokenStream) -> TokenStream { asset::asset_attrib(attrib_input, input) }

#[proc_macro]
pub fn pascal_to_title(input: TokenStream) -> TokenStream { case_conv::pascal_to_title(input) }