use std::collections::HashMap;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr};

// TODO: make enum_prop_opt

// Fancy enum options: names, name_hashes; others?

// supports, for each variant: [enum_prop(key = "value")
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

    let mut unit_variant_pairs = Vec::new();
    let mut match_variants_idents = Vec::new();
    let mut match_variant_indices = Vec::new();
    let mut props = HashMap::new();

    let mut mandatory_key_counts = HashMap::new();

    for (vindex, variant) in variants.iter().enumerate()
    {
        let variant_ident = &variant.ident;
        
        // only emit if all variants are units?
        if let Fields::Unit = variant.fields
        {
            unit_variant_pairs.push(quote!( (stringify!(#variant_ident), Self::#variant_ident) ));
        }

        // todo: name override attr
        match_variants_idents.push(match variant.fields
        {
            Fields::Named(_) => quote!( Self::#variant_ident{..} => stringify!(#variant_ident) ),
            Fields::Unnamed(_) => quote!( Self::#variant_ident(..) => stringify!(#variant_ident) ),
            Fields::Unit => quote!( Self::#variant_ident => stringify!(#variant_ident) ),
        });
        match_variant_indices.push(match variant.fields
        {
            Fields::Named(_) => quote!( Self::#variant_ident{..} => #vindex ),
            Fields::Unnamed(_) => quote!( Self::#variant_ident(..) => #vindex ),
            Fields::Unit => quote!( Self::#variant_ident => #vindex ),
        });

        // if let Fields::Unit = variant.fields
        // {
        //     try_froms.push(quote!( stringify!(#variant_ident) => Ok(Self::#variant_ident) ));
        // }

        for attr in variant.attrs.iter()
        {
            // TODO: optional props
            // TODO: support other basic types besides strings for prop vals
            if !attr.path().is_ident("enum_prop") { continue; }

            attr.parse_nested_meta(|meta|
            {
                let prop_key = meta.path.get_ident().ok_or(meta.error("Missing property key"))?;
                let prop_val: LitStr = meta.value()?.parse()?;

                *mandatory_key_counts.entry(prop_key.to_owned()).or_insert(0) += 1;

                let method_name = prop_key.clone();
                let prop = props.entry(method_name).or_insert(Vec::<proc_macro2::TokenStream>::new());
                prop.push(match variant.fields
                {
                    Fields::Named(_) => quote!(Self::#variant_ident{..} => #prop_val),
                    Fields::Unnamed(_) => quote!(Self::#variant_ident(..) => #prop_val),
                    Fields::Unit => quote!(Self::#variant_ident => #prop_val),
                });

                Ok(())
            }).expect("Failed to parse enum_prop");
        }
    }

    for (key, count) in mandatory_key_counts
    {
        if count != match_variants_idents.len()
        {
            panic!("Property key '{}' is used in {} variant(s), but not in all {}", key, count, match_variants_idents.len());
        }
    }

    let methods = props.iter().map(|(prop_key, prop_values)|
    {
        quote!
        {
            pub const fn #prop_key(&self) -> &'static str
            {
                match self { #(#prop_values,)* }
            }
        }
    });

    let variants_count = match_variants_idents.len();

    // Expand the generated methods
    let expanded = quote!
    {
        impl #type_name
        {
            pub const fn variant_name(&self) -> &'static str { match self { #(#match_variants_idents),* } }
            pub const fn variant_index(&self) -> usize { match self { #(#match_variant_indices),* } }
            pub const fn variant_count() -> usize { #variants_count }
            pub const fn unit_variants() -> &'static [(&'static str, Self)]
            {
                &[ #(#unit_variant_pairs),* ]
            }

            #(#methods)*
        }
    };

    expanded.into()
}