use std::collections::HashMap;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr};

// TODO: make enum_prop_opt and enum_prop (checks all entries)

pub fn fancy_enum(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    let derive = parse_macro_input!(input as DeriveInput);
    let type_name = &derive.ident;

    let variants = if let Data::Enum(ref data_enum) = derive.data
    {
        &data_enum.variants
    }
    else
    {
        panic!("#[derive(FancyEnum)] can only be used with enums");
    };

    // Store generated methods for each property

    let mut variants_idents = Vec::new();
    let mut props = HashMap::new();

    for variant in variants
    {
        let variant_ident = &variant.ident;

        // todo: name override attr
        variants_idents.push(match variant.fields
        {
            Fields::Named(_) => quote!(Self::#variant_ident{..} => stringify!(#variant_ident)),
            Fields::Unnamed(_) => quote!(Self::#variant_ident(..) => stringify!(#variant_ident)),
            Fields::Unit => quote!(Self::#variant_ident => stringify!(#variant_ident)),
        });

        for attr in &variant.attrs
        {
            if !attr.path().is_ident("enum_prop") { continue; }

            attr.parse_nested_meta(|meta|
            {
                let prop_key = meta.path.get_ident().ok_or(meta.error("Missing property key"))?;
                let prop_val: LitStr = meta.value()?.parse()?;

                let method_name = prop_key.clone();
                let prop = props.entry(method_name).or_insert(Vec::<proc_macro2::TokenStream>::new());
                // prop.push(quote!(Self::#variant_ident => #prop_val).into());
                prop.push(match variant.fields
                {
                    Fields::Named(_) => quote!(Self::#variant_ident{..} => Some(#prop_val)),
                    Fields::Unnamed(_) => quote!(Self::#variant_ident(..) => Some(#prop_val)),
                    Fields::Unit => quote!(Self::#variant_ident => Some(#prop_val)),
                });

                Ok(())
            }).expect("Failed to parse enum_prop");
        }
    }

    let methods = props.iter().map(|(prop_key, prop_values)|
    {
        quote!
        {
            pub const fn #prop_key(&self) -> Option<&'static str>
            {
                match self
                {
                    #(#prop_values,)*
                    _ => None,
                }
            }
        }
    });

    let variants_count = variants_idents.len();

    // Expand the generated methods
    let expanded = quote!
    {
        impl #type_name
        {
            pub const fn variant_name(&self) -> &'static str
            {
                match self
                {
                    #(#variants_idents),*
                }
            }

            pub const fn variant_count() -> usize
            {
                #variants_count
            }

            #(#methods)*
        }
    };

    expanded.into()
}