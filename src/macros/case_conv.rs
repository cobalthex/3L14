use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

pub fn pascal_to_title(input: TokenStream) -> TokenStream
{
    let input = input.to_string();
    let raw = if input.starts_with('"') && input.ends_with('"')
    {
        // Strip quotes if it's a string literal
        input.trim_matches('"').to_owned()
    }
    else
    {
        input
    };

    let mut result = String::new();
    for (i, c) in raw.chars().enumerate()
    {
        if i > 0 && c.is_uppercase()
        {
            result.push(' ');
        }
        result.push(c);
    }

    // let lower = result.to_lowercase();
    // let sentence_case = lower
    //     .chars()
    //     .enumerate()
    //     .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
    //     .collect::<String>();
    // TokenStream::from(quote!(#sentence_case))
    TokenStream::from(quote!(#result))
}