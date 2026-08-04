use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

pub fn enum_from_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    let derive = parse_macro_input!(input as DeriveInput);
    let type_name = &derive.ident;

    let variants = if let Data::Enum(ref data_enum) = derive.data
    {
        &data_enum.variants
    } else {
        panic!("#[derive(FancyEnum)] can only be used with enums");
    };

    // TODO: validate that all variants are units?

    let variants = variants.iter().map(|v| &v.ident);

    // TODO: case insensitive?

    quote!
    {
        impl From<&str> for #type_name
        {
            fn from(s: &str) -> Self
            {
                match s
                {
                    #(stringify!(#variants) => Self::#variants,)*
                    _ => panic!("Unknown enum variant: {}", s),
                }
            }
        }
    }.into()
}