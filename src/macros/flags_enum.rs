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
            pub const fn bits_used() -> u8
            {
                #bits_used
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
        impl ::std::ops::BitAnd for #type_name
        {
            type Output = Self;
            fn bitand(self, other: Self) -> Self::Output
            {
                let n = (self as #repr_size) & (other as #repr_size);
                unsafe { ::std::mem::transmute(n) }
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
    };
    expanded.into()
}
