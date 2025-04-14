use syn::Attribute;

pub fn has_derive(which_derive: &str, attrs: &Vec<Attribute>) -> bool
{
    attrs.iter().any(|a|
    {
        if !a.path().is_ident("derive") { return false; };
        let mut found_derive = false;
        let _ = a.parse_nested_meta(|meta|
        {
            if meta.path.is_ident(which_derive) { found_derive = true; }
            Ok(())
        });
        found_derive
    })
}