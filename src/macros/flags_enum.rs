use std::ops::{BitAnd, Shr};
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

    for variant in variants
    {
        let variant_name = &variant.ident;

        let Fields::Unit = variant.fields else { panic!("#[derive(Flags)]{type_name} must have unit variants only"); };
        let Some((_, disc))  = &variant.discriminant else { panic!("{type_name} variant {variant_name} must have a discriminant"); };

        let Expr::Lit(ExprLit { lit: Lit::Int(int), .. }) = disc else { continue; };
        let Ok(n) = int.base10_parse::<u128>() else { continue; };
        // panic!("{type_name}::{variant_name} discriminant = {} {}", int, int.base10_digits());

        known_values |= n;
    }

    // negative values don't seem to count towards size of enum...

    if known_values == 0
    {
        panic!("{type_name} had no positive integer discriminants");
    }

    let bits_used = (u128::BITS - known_values.leading_zeros()) as u8;

    let expanded = quote!
    {
        impl #type_name
        {
            pub const fn has_flag(&self, flag: Self) -> bool
            {
                let n = (*self as #repr_size) & (flag as #repr_size);
                n == (flag as #repr_size)
            }
            pub const fn all_flags() -> Self
            {
                unsafe { ::std::mem::transmute(#known_values as #repr_size) }
            }
            // The number of bits required to represent this enum, this includes unused bits in the middle (e.g. bits 1001 = 4 used)
            pub const fn bits_used() -> u8
            {
                #bits_used
            }

            // iterate all the flags active in this enum
            pub const fn iter_set_flags(self) -> ::nab_3l14::enum_helpers::FlagsEnumIter<Self>
            {
                ::nab_3l14::enum_helpers::FlagsEnumIter::new(self)
            }
        }
        impl ::nab_3l14::enum_helpers::FlagsEnum<#type_name> for #type_name
        {
            #[inline] fn bits_used_trait() -> u8 { Self::bits_used() }
            #[inline] fn get_flag_for_bit(self, bit: u8) -> Option<#type_name>
            {
                let val = (1 as #repr_size) << bit;
                if ((self as #repr_size) & val) == val
                {
                    Some(unsafe { std::mem::transmute(val) })
                }
                else
                {
                    None
                }
            }
        }
        impl ::std::ops::BitOr for #type_name
        {
            type Output = Self;
            fn bitor(self, other: Self) -> Self::Output
            {
                let n = (self as #repr_size) | (other as #repr_size);
                unsafe { ::std::mem::transmute(n) }
            }
        }
        impl ::std::ops::BitOrAssign for #type_name
        {
            fn bitor_assign(&mut self, other: Self)
            {
                *self = (*self | other) as Self;
            }
        }
        impl ::std::ops::BitAnd for #type_name
        {
            type Output = Self;
            fn bitand(self, other: Self) -> Self::Output
            {
                let n = (self as #repr_size) & (other as #repr_size);
                unsafe { ::std::mem::transmute(n) }
            }
        }
        impl ::std::ops::BitAndAssign for #type_name
        {
            fn bitand_assign(&mut self, other: Self)
            {
                *self = (*self & other) as Self;
            }
        }
        impl ::std::ops::BitXor for #type_name
        {
            type Output = Self;
            fn bitxor(self, other: Self) -> Self::Output
            {
                let n = (self as #repr_size) ^ (other as #repr_size);
                unsafe { ::std::mem::transmute(n) }
            }
        }
        impl ::std::ops::BitXorAssign for #type_name
        {
            fn bitxor_assign(&mut self, other: Self)
            {
                *self = (*self ^ other) as Self;
            }
        }
        impl ::std::ops::Not for #type_name
        {
            type Output = Self;
            fn not(self) -> Self::Output
            {
                let n = !(self as #repr_size);
                unsafe { ::std::mem::transmute(n) }
            }
        }
        impl From<#type_name> for #repr_size
        {
            fn from(other: #type_name) -> Self
            {
                other as Self
            }
        }
        impl PartialEq for #type_name
        {
            fn eq(&self, other: &Self) -> bool { (*self as #repr_size) == (*other as #repr_size) }
        }
        impl Eq for #type_name { }
        impl Clone for #type_name
        {
            fn clone(&self) -> Self { *self }
        }
        impl Copy for #type_name { }
        // implement Ord/PartialOrd ?
    };
    expanded.into()
}
