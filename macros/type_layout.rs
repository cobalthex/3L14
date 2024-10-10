// use std::intrinsics::type_name;
// use proc_macro2::TokenStream;
// use quote::quote;
// use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};
// use metrohash::MetroHash64;
// 
// const fn hash_field_type(ty: &Type) -> TokenStream
// {
//     match ty
//     {
//         // Built-in types can be directly converted to string and hashed
//         Type::Path(type_path)
//             if type_path.path.is_ident(type_name::<bool>())
//             || type_path.path.is_ident(type_name::<i8>())
//             || type_path.path.is_ident(type_name::<i16>())
//             || type_path.path.is_ident(type_name::<i32>())
//             || type_path.path.is_ident(type_name::<i64>())
//             || type_path.path.is_ident(type_name::<i128>())
//             || type_path.path.is_ident(type_name::<isize>())
//             || type_path.path.is_ident(type_name::<u8>())
//             || type_path.path.is_ident(type_name::<u16>())
//             || type_path.path.is_ident(type_name::<u32>())
//             || type_path.path.is_ident(type_name::<u64>())
//             || type_path.path.is_ident(type_name::<u128>())
//             || type_path.path.is_ident(type_name::<usize>())
//             || type_path.path.is_ident(type_name::<f32>())
//             || type_path.path.is_ident(type_name::<f64>())
//             || type_path.path.is_ident(type_name::<bool>())
//             || type_path.path.is_ident(type_name::<()>())
//             || type_path.path.is_ident(type_name::<char>())
//             || type_path.path.is_ident(type_name::<str>())
//             || type_path.path.is_ident(type_name::<String>()) => // fn type?
//         {
//             let type_name = type_path.path.segments[0].ident.to_string();
//             quote!
//             {
//                 #type_name.hash(&mut hasher);
//             }
//         }
//         // If the field type is a custom type (struct/enum), recursively call its hash function
//         Type::Path(type_path) =>
//         {
//             let type_name = &type_path.path.segments[0].ident;
//             quote!
//             {
//                 #type_name::hash_type_structure().hash(&mut hasher);
//             }
//         }
// 
//         // Handle other possible types, like arrays, options, tuples, etc.
//         _ =>
//         {
//             todo!("Handling of this type is not implemented yet")
//         }
//     }
// }
// 
// #[proc_macro_derive(HashType)]
// pub fn derive_hash_type(input: proc_macro::TokenStream) -> proc_macro::TokenStream
// {
//     let input = parse_macro_input!(input as DeriveInput);
//     let name = &input.ident;
// 
//     let expanded = match input.data
//     {
//         // Handle structs
//         Data::Struct(ref data_struct) =>
//         {
//             let fields = data_struct.fields.iter().map(|field|
//             {
//                 let field_name = field.ident.as_ref().unwrap().to_string();
//                 let field_type = hash_field_type(&field.ty);
//                 quote!
//                 {
//                     #field_name.hash(&mut hasher);
//                     #field_type
//                 }
//             });
//             quote!
//             {
//                 impl HashType for #name
//                 {
//                     fn hash_type_structure() -> u64
//                     {
//                         let mut hasher = std::collections::hash_map::DefaultHasher::new();
//                         // Hash the struct name
//                         stringify!(#name).hash(&mut hasher);
//                         // Hash the fields recursively
//                         #(#fields)*
//                         hasher.finish()
//                     }
//                 }
//             }
//         }
//         // Handle enums
//         Data::Enum(ref data_enum) =>
//         {
//             let variants = data_enum.variants.iter().map(|variant|
//             {
//                 let variant_name = variant.ident.to_string();
//                 let variant_fields = match &variant.fields
//                 {
//                     Fields::Named(fields) => fields.named.iter().map(|field|
//                     {
//                         let field_name = field.ident.as_ref().unwrap().to_string();
//                         let field_type = hash_field_type(&field.ty);
//                         quote!
//                         {
//                             #field_name.hash(&mut hasher);
//                             #field_type
//                         }
//                     }),
//                     Fields::Unnamed(fields) => fields.unnamed.iter().enumerate().map(|(i, field)|
//                         {
//                         let field_name = format!("unnamed_field_{}", i);
//                         let field_type = hash_field_type(&field.ty);
//                         quote!
//                         {
//                             #field_name.hash(&mut hasher);
//                             #field_type
//                         }
//                     }),
//                     Fields::Unit => vec![].into_iter(),
//                 };
//                 quote!
//                 {
//                     #variant_name.hash(&mut hasher);
//                     #(#variant_fields)*
//                 }
//             });
//             quote!
//             {
//                 impl HashType for #name
//                 {
//                     fn hash_type_structure() -> u64
//                     {
//                         let mut hasher = std::collections::hash_map::DefaultHasher::new();
//                         // Hash the enum name
//                         stringify!(#name).hash(&mut hasher);
//                         // Hash the variant names and fields recursively
//                         #(#variants)*
//                         hasher.finish()
//                     }
//                 }
//             }
//         }
//         // Unions can be left unimplemented
//         Data::Union(_) =>
//             {
//             unimplemented!("Unions are not supported")
//         }
//     };
// 
//     TokenStream::from(expanded)
// }