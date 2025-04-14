use derives::{asset_derive, fancy_enum, flags_enum, type_layout_hash};

mod derives;
mod has_derive;

// FancyEnum adds .variant_ident() and for each #[enum_prop(k=v)] a method k() returning v
#[proc_macro_derive(FancyEnum, attributes(enum_prop))]
pub fn fancy_enum_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    fancy_enum::fancy_enum(input)
}

// Adds standard bit ops and .has_flag(..) to enums
#[proc_macro_derive(Flags)]
pub fn flags_enum_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    flags_enum::flags_enum(input)
}

#[proc_macro_derive(LayoutHash)]
pub fn type_layout_hash_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream { type_layout_hash::type_layout_hash(input) }

#[proc_macro_derive(Asset)] // todo: better name?
pub fn asset_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream { asset_derive::asset_derive(input) }

