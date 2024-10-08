use quote::quote;
use std::collections::HashMap;
use syn::{self, parse_macro_input, Data, DeriveInput, LitStr};

#[proc_macro_derive(EnumWithProps, attributes(enum_prop))]
pub fn enum_props_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    let derive = parse_macro_input!(input as DeriveInput);
    let type_name = &derive.ident;

    let variants = if let Data::Enum(ref data_enum) = derive.data
    {
        &data_enum.variants
    }
    else
    {
        panic!("#[derive(EnumWithProps)] can only be used with enums");
    };

    // Store generated methods for each property

    let mut props = HashMap::new();

    for variant in variants
    {
        let variant_ident = &variant.ident;

        for attr in &variant.attrs
        {
            if !attr.path().is_ident("enum_prop") { continue; }

            attr.parse_nested_meta(|meta|
            {
                let prop_key = meta.path.get_ident().ok_or(meta.error("Missing property key"))?;
                let prop_val: LitStr = meta.value()?.parse()?;

                let method_name = prop_key.clone();
                let prop = props.entry(method_name).or_insert(Vec::<proc_macro2::TokenStream>::new());
                prop.push(quote!(Self::#variant_ident => #prop_val).into());

                Ok(())
            }).expect("Failed to parse enum_prop");
        }
    }

    let methods = props.iter().map(|(prop_key, prop_values)|
    {
        quote!
        {
            pub const fn #prop_key(&self) -> &'static str
            {
                match self
                {
                    #(#prop_values,)*
                    _ => panic!("This variant does not have a property for {}", #prop_key),
                }
            }
        }
    });

    // Expand the generated methods
    let expanded = quote!
    {
        impl #type_name
        {
            #(#methods)*
        }
    };

    proc_macro::TokenStream::from(expanded)
}
