use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, DataStruct, ExprStruct, Fields, ItemStruct, Meta, Token, Type};

fn type_path_has_ident(ty: &Type, ident: &Ident) -> bool
{
    let Type::Path(ty_path) = ty else { return false; };

    for seg in ty_path.path.segments.iter()
    {
        if seg.ident == *ident
        {
            return true;
        }

        // TODO: recursive search of template args, ideal for iterables
        // match &seg.arguments
        // {
        //     PathArguments::None => {}
        //     PathArguments::AngleBracketed(ab) =>
        //     {
        //         for arg in ab.args.iter()
        //         {
        //             match arg
        //             {
        //                 GenericArgument::Type(ty) => if type_path_has_ident(ty, ident) { return true; }
        //                 _ => {}
        //             }
        //         }
        //     }
        //     PathArguments::Parenthesized(paren) =>
        //     {
        //         for arg in paren.inputs.iter()
        //         {
        //             if type_path_has_ident(arg, ident) { return true; }
        //         }
        //     }
        // }
    }

    false
}

struct AssetAttribArgs(Vec<Meta>);
impl Parse for AssetAttribArgs
{
    fn parse(input: ParseStream) -> syn::Result<Self>
    {
        let mut args = Vec::new();
        while !input.is_empty()
        {
            args.push(input.parse()?);
            if input.peek(Token![,])
            {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(AssetAttribArgs(args))
    }
}

// Derive the Asset trait automatically.
// Optional attributes include:
//  debug_type=<TypeName>
pub fn asset_attrib(attrib_input: TokenStream, input: TokenStream) -> TokenStream
{
    let attrib_args = parse_macro_input!(attrib_input as AssetAttribArgs);

    let mut maybe_debug_type = None;

    for attrib in attrib_args.0.iter()
    {
        let Meta::NameValue(name_value) = attrib
            else { panic!("#[asset] Expected a name=value attribute, got {:?}", attrib.to_token_stream()) };

        if name_value.path.is_ident("debug_type")
        {
            if maybe_debug_type.is_some() { panic!("#[asset] debug_type specified multiple times"); }
            maybe_debug_type = Some(name_value.value.clone());
        }
        else
        {
            panic!("#[asset] Invalid attribute: {:?}", name_value.path.to_token_stream());
        }
    }

    let strukt = parse_macro_input!(input as ItemStruct);
    let struct_name = strukt.ident.clone();

    let mut asset_handles = Vec::new();

    match &strukt.fields
    {
        Fields::Named(members) =>
        {
            for field in members.named.iter()
            {
                if type_path_has_ident(&field.ty, &format_ident!("Ash"))
                {
                    asset_handles.push(field.ident.clone().unwrap());
                }
            }
        },
        Fields::Unnamed(members) =>
        {
            // TODO
        },
        Fields::Unit => { }
    }

    let debug_type = maybe_debug_type.unwrap_or_else(|| syn::parse_str("()").unwrap());

    let mut handle_refs = quote!{ #(self.#asset_handles.all_dependencies_loaded())&&* };
    if handle_refs.is_empty() { handle_refs = quote!(true) };

    (quote!
    {
        #strukt
        impl ::asset_3l14::Asset for #struct_name
        {
            type DebugData = #debug_type;
            fn asset_type() -> ::asset_3l14::AssetTypeId { ::asset_3l14::AssetTypeId::#struct_name }
            fn all_dependencies_loaded(&self) -> bool
            {
                #handle_refs
            }
        }
    }).into()
}