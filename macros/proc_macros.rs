mod fancy_enum;

// FancyEnum adds .variant_ident() and for each #[enum_prop(k=v)] a method k() returning v
#[proc_macro_derive(FancyEnum, attributes(enum_prop))]
pub fn enum_props_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    fancy_enum::fancy_enum(input)
}
