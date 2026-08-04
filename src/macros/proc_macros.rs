use proc_macro::TokenStream;
use syn::token::Token;
use derives::{fancy_enum, type_layout_hash};
use attribs::asset;
use crate::derives::{circuit_block, enum_from_str};

mod derives;
mod has_derive;
mod attribs;
mod case_conv;
mod ident_hash;

// FancyEnum adds .variant_ident()
// for each #[enum_prop(k=v)] a method k() returning v 
#[proc_macro_derive(FancyEnum, attributes(enum_prop))]
pub fn derive_fancy_enum(input: TokenStream) -> TokenStream
{
    fancy_enum::fancy_enum(input)
}

// TODO: combine with FancyEnum as an option? -- might need to be attribute macro?
#[proc_macro_derive(EnumFromStr)]
pub fn derive_enum_from_str(input: TokenStream) -> TokenStream { enum_from_str::enum_from_str(input) }

#[proc_macro_derive(LayoutHash)]
pub fn derive_type_layout_hash(input: TokenStream) -> TokenStream { type_layout_hash::type_layout_hash(input) }

#[proc_macro_derive(CircuitBlock)]
pub fn derive_circuit_block(input: TokenStream) -> TokenStream { circuit_block::circuit_block(input) }

#[proc_macro_attribute] // todo: better name?
pub fn asset(attrib_input: TokenStream, input: TokenStream) -> TokenStream { asset::asset_attrib(attrib_input, input) }

#[proc_macro]
pub fn pascal_to_title(input: TokenStream) -> TokenStream { case_conv::pascal_to_title(input) }

#[proc_macro]
pub fn ident_hash32(input: TokenStream) -> TokenStream { ident_hash::ident_hash32(input) }