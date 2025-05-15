use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Lit};

fn get_repr_size(attrs: &[Attribute]) -> Option<Ident>
{
    attrs.iter()
        .filter_map(|attr|
        {
            if !attr.path().is_ident("repr") { return None; }

            let mut repr = None;
            let _ = attr.parse_nested_meta(|meta|
            {
                // inefficient
                if
                    meta.path.is_ident("u8") ||
                    meta.path.is_ident("u16") ||
                    meta.path.is_ident("u32") ||
                    meta.path.is_ident("u64") ||
                    meta.path.is_ident("u128") ||
                    meta.path.is_ident("usize") ||
                    meta.path.is_ident("i8") ||
                    meta.path.is_ident("i16") ||
                    meta.path.is_ident("i32") ||
                    meta.path.is_ident("i64") ||
                    meta.path.is_ident("i128") ||
                    meta.path.is_ident("isize")
                {
                    repr = Some(meta.path.get_ident().unwrap().clone());
                }
                Ok(())
            });
            repr
        }).next()
}

pub fn flags_enum(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    let derive = parse_macro_input!(input as DeriveInput);
    let type_name = &derive.ident;

    let repr_size = get_repr_size(&derive.attrs).expect("Flags enums must have an integer repr");

    let variants = if let Data::Enum(ref data_enum) = derive.data
    {
        &data_enum.variants
    } else {
        panic!("#[derive(Flags)] can only be used with enums");
    };

    let mut known_values = 0u128;

    let mut variant_names = Vec::new();

    for variant in variants
    {
        let variant_ident = &variant.ident;
        variant_names.push(quote!(Self::#variant_ident => stringify!(#variant_ident)));

        let Fields::Unit = variant.fields else { panic!("#[derive(Flags)]{type_name} must have unit variants only"); };
        let Some((_, disc))  = &variant.discriminant else { panic!("{type_name}::{variant_ident} must have a discriminant"); };

        let Expr::Lit(ExprLit { lit: Lit::Int(int), .. }) = disc else { continue; };
        let Ok(variant_value) = int.base10_parse::<u128>() else { continue; };
        // panic!("{type_name}::{variant_name} discriminant = {} {}", int, int.base10_digits());

        if variant_value == 0
        {
            panic!("{type_name}::{variant_ident} cannot be 0. Use {type_name}::none() instead");
        }

        // allow for custom attrib to enable this?
        if (variant_value & known_values) != 0
        {
            panic!("{type_name}::{variant_ident} has a discriminant that overlaps with another variant");
        }

        // TODO: check for duplicates

        known_values |= variant_value;
    }
    // negative values don't seem to count towards size of enum...

    if known_values == 0
    {
        panic!("{type_name} had no positive integer discriminants");
    }

    let bits_used = (u128::BITS - known_values.leading_zeros()) as u8;

    // let found_debug_derive = has_derive("Debug", &derive.attrs);
    // TODO: conditionally disable debug drive if already present? (or panic)

    let expanded = quote!
    {
        impl #type_name
        {
            // Create an instance of this enum with no values set
            pub const fn none() -> Self { unsafe { ::core::mem::transmute(0 as #repr_size) } }

            // check if one or more flags are set on this enum
            pub const fn has_flag(&self, flag: Self) -> bool
            {
                let n = (*self as #repr_size) & (flag as #repr_size);
                n == (flag as #repr_size)
            }
            // return a value with all of the available bits set
            pub const fn all_flags() -> Self
            {
                unsafe { ::core::mem::transmute(#known_values as #repr_size) }
            }
            // The number of bits required to represent this enum, this includes unused bits in the middle (e.g. bits 1001 = 4 used)
            pub const fn bits_used() -> u8
            {
                #bits_used
            }

            // iterate all the flags active in this enum
            pub const fn iter_set_flags(self) -> ::nab_3l14::FlagsEnumIter<Self>
            {
                ::nab_3l14::FlagsEnumIter::new(self)
            }
        }
        impl ::nab_3l14::FlagsEnum<#type_name> for #type_name
        {
            type Repr = #repr_size;

            #[inline] fn bits_used_trait() -> u8 { Self::bits_used() }
            #[inline] fn get_flag_for_bit(self, bit: u8) -> ::core::option::Option<#type_name>
            {
                let val = (1 as #repr_size) << bit;
                if ((self as #repr_size) & val) == val
                {
                    Some(unsafe { ::core::mem::transmute(val) })
                }
                else
                {
                    None
                }
            }
        }
        impl ::core::ops::BitOr for #type_name
        {
            type Output = Self;
            fn bitor(self, other: Self) -> Self::Output
            {
                let n = (self as #repr_size) | (other as #repr_size);
                unsafe { ::core::mem::transmute(n) }
            }
        }
        impl ::core::ops::BitOrAssign for #type_name
        {
            fn bitor_assign(&mut self, other: Self)
            {
                *self = (*self | other) as Self;
            }
        }
        impl ::core::ops::BitAnd for #type_name
        {
            type Output = Self;
            fn bitand(self, other: Self) -> Self::Output
            {
                let n = (self as #repr_size) & (other as #repr_size);
                unsafe { ::core::mem::transmute(n) }
            }
        }
        impl ::core::ops::BitAndAssign for #type_name
        {
            fn bitand_assign(&mut self, other: Self)
            {
                *self = (*self & other) as Self;
            }
        }
        impl ::core::ops::BitXor for #type_name
        {
            type Output = Self;
            fn bitxor(self, other: Self) -> Self::Output
            {
                let n = (self as #repr_size) ^ (other as #repr_size);
                unsafe { ::core::mem::transmute(n) }
            }
        }
        impl ::core::ops::BitXorAssign for #type_name
        {
            fn bitxor_assign(&mut self, other: Self)
            {
                *self = (*self ^ other) as Self;
            }
        }
        impl ::core::ops::Not for #type_name
        {
            type Output = Self;
            fn not(self) -> Self::Output
            {
                let n = !(self as #repr_size);
                unsafe { ::core::mem::transmute(n) }
            }
        }
        impl ::core::convert::From<#type_name> for #repr_size
        {
            fn from(other: #type_name) -> Self
            {
                other as Self
            }
        }
        impl ::core::cmp::PartialEq for #type_name
        {
            fn eq(&self, other: &Self) -> bool { (*self as #repr_size) == (*other as #repr_size) }
        }
        impl ::core::cmp::Eq for #type_name { }
        impl ::core::clone::Clone for #type_name
        {
            fn clone(&self) -> Self { *self }
        }
        impl ::core::marker::Copy for #type_name { }
        // implement Ord/PartialOrd ?

        //try_from (validates all values)?

        impl ::core::convert::From<#repr_size> for #type_name
        {
            fn from(value: #repr_size) -> Self
            {
                unsafe { std::mem::transmute(value) }
            }
        }

        // TODO: check if (and error) if deriving bitcode encode/decode, currently incompatible

        // TODO: conditionally enable?
        impl ::core::fmt::Debug for #type_name
        {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::result::Result<(), ::core::fmt::Error>
            {
                // TODO: alt formatting
                f.write_fmt(format_args!("{}[", <Self as nab_3l14::utils::ShortTypeName>::short_type_name()))?;

                let mut rest = false;
                for flag in self.iter_set_flags()
                {
                    if (rest)
                    {
                        f.write_str("|")?;
                    }
                    rest = true;

                    f.write_str(match flag
                    {
                        #(#variant_names,)*
                    })?;
                }
                f.write_str("]")?;
                Ok(())
            }
        }
    };
    expanded.into()
}
