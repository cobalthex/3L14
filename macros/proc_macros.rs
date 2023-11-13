use quote::quote;
use syn::{self, Ident, parse_macro_input};

// todo: use send/sync types only?

#[proc_macro_derive(GlobalSingleton)]
pub fn global_singleton_derive_impl(input: proc_macro::TokenStream) -> proc_macro::TokenStream
{
    // Parse the input tokens into a syntax tree
    let parsed = parse_macro_input!(input as syn::DeriveInput);

    let for_ty = &parsed.ident;

    // let global_span = for_ty.span().unwrap().start();
    let global_span = for_ty.span();
    let global = Ident::new(&format!("g_{}", for_ty), global_span);

    quote!
    {
        #[warn(non_upper_case_globals)]
        static #global: #for_ty = #for_ty::new();
        impl GlobalSingleton for #for_ty
        {
            fn get<'s>() -> &'s Self { &#global }
        }
    }.into()
}
