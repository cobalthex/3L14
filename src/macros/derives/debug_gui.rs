use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Index, Visibility};

fn is_primitive_path(path: &syn::Path) -> bool
{
    if let Some(segment) = path.segments.last()
    {
        let ident_str = segment.ident.to_string();
        matches!(
            ident_str.as_str(),
            "bool"
                | "char"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "f32"
                | "f64"
                | "str"
                | "String"
        )
    }
    else
    {
        false
    }
}

fn is_primitive(ty: &syn::Type) -> bool
{
    match ty
    {
        syn::Type::Path(type_path) => is_primitive_path(&type_path.path),
        syn::Type::Reference(type_ref) => is_primitive(&type_ref.elem),
        syn::Type::Group(type_group) => is_primitive(&type_group.elem),
        syn::Type::Paren(type_paren) => is_primitive(&type_paren.elem),
        _ => false,
    }
}

fn has_skip_gui(attrs: &[syn::Attribute]) -> bool
{
    attrs.iter().any(|attr| attr.path().is_ident("skip_gui"))
}

pub fn debug_gui(input: TokenStream) -> TokenStream
{
    let derive_input = parse_macro_input!(input as DeriveInput);
    let type_name = &derive_input.ident;

    let debug_crate = match std::env::var("CARGO_PKG_NAME").as_deref()
    {
        Ok("debug_3l14") => quote!(crate),
        _ => quote!(::debug_3l14),
    };
    let debug_gui_trait = quote!(#debug_crate::debug_gui::DebugGui);

    let (impl_generics, ty_generics, where_clause) = derive_input.generics.split_for_impl();

    let grid_id_str = format!("{}_debug_gui_grid", type_name);

    let debug_gui_body = match &derive_input.data
    {
        Data::Struct(data_struct) =>
        {
            let mut rows = Vec::new();

            match &data_struct.fields
            {
                Fields::Named(fields_named) =>
                {
                    for field in &fields_named.named
                    {
                        if has_skip_gui(&field.attrs)
                        {
                            continue;
                        }

                        if matches!(field.vis, Visibility::Public(_))
                        {
                            let field_ident = field.ident.as_ref().unwrap();
                            let field_name_str = field_ident.to_string();

                            if is_primitive(&field.ty)
                            {
                                rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    ui.label(format!("{}", self.#field_ident));
                                    ui.end_row();
                                });
                            }
                            else
                            {
                                rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    self.#field_ident.debug_gui(ui);
                                    ui.end_row();
                                });
                            }
                        }
                    }
                }
                Fields::Unnamed(fields_unnamed) =>
                {
                    for (i, field) in fields_unnamed.unnamed.iter().enumerate()
                    {
                        if has_skip_gui(&field.attrs)
                        {
                            continue;
                        }

                        if matches!(field.vis, Visibility::Public(_))
                        {
                            let index = Index::from(i);
                            let field_name_str = i.to_string();

                            if is_primitive(&field.ty)
                            {
                                rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    ui.label(format!("{}", self.#index));
                                    ui.end_row();
                                });
                            }
                            else
                            {
                                rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    self.#index.debug_gui(ui);
                                    ui.end_row();
                                });
                            }
                        }
                    }
                }
                Fields::Unit => {}
            }

            quote!
            {
                ::egui::Grid::new(#grid_id_str)
                    .striped(true)
                    .show(ui, |ui|
                    {
                        #(#rows)*
                    });
            }
        }
        Data::Enum(data_enum) =>
        {
            let mut match_arms = Vec::new();

            for variant in &data_enum.variants
            {
                let variant_ident = &variant.ident;
                let variant_name_str = variant_ident.to_string();

                if has_skip_gui(&variant.attrs)
                {
                    match &variant.fields
                    {
                        Fields::Named(_) => match_arms.push(quote!(Self::#variant_ident { .. } => {})),
                        Fields::Unnamed(_) => match_arms.push(quote!(Self::#variant_ident(..) => {})),
                        Fields::Unit => match_arms.push(quote!(Self::#variant_ident => {})),
                    }
                    continue;
                }

                match &variant.fields
                {
                    Fields::Named(fields_named) =>
                    {
                        let mut field_patterns = Vec::new();
                        let mut field_rows = Vec::new();

                        for field in &fields_named.named
                        {
                            let field_ident = field.ident.as_ref().unwrap();
                            let field_name_str = field_ident.to_string();

                            if has_skip_gui(&field.attrs)
                            {
                                field_patterns.push(quote!(#field_ident: _));
                                continue;
                            }

                            field_patterns.push(quote!(#field_ident));

                            if is_primitive(&field.ty)
                            {
                                field_rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    ui.label(format!("{}", #field_ident));
                                    ui.end_row();
                                });
                            }
                            else
                            {
                                field_rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    #field_ident.debug_gui(ui);
                                    ui.end_row();
                                });
                            }
                        }

                        match_arms.push(quote!
                        {
                            Self::#variant_ident { #(#field_patterns),* } =>
                            {
                                ui.label("Variant");
                                ui.label(#variant_name_str);
                                ui.end_row();
                                #(#field_rows)*
                            }
                        });
                    }
                    Fields::Unnamed(fields_unnamed) =>
                    {
                        let mut field_patterns = Vec::new();
                        let mut field_rows = Vec::new();

                        for (i, field) in fields_unnamed.unnamed.iter().enumerate()
                        {
                            let field_name_str = i.to_string();

                            if has_skip_gui(&field.attrs)
                            {
                                field_patterns.push(quote!(_));
                                continue;
                            }

                            let binding_ident = quote::format_ident!("f_{}", i);
                            field_patterns.push(quote!(#binding_ident));

                            if is_primitive(&field.ty)
                            {
                                field_rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    ui.label(format!("{}", #binding_ident));
                                    ui.end_row();
                                });
                            }
                            else
                            {
                                field_rows.push(quote!
                                {
                                    ui.label(#field_name_str);
                                    #binding_ident.debug_gui(ui);
                                    ui.end_row();
                                });
                            }
                        }

                        match_arms.push(quote!
                        {
                            Self::#variant_ident(#(#field_patterns),*) =>
                            {
                                ui.label("Variant");
                                ui.label(#variant_name_str);
                                ui.end_row();
                                #(#field_rows)*
                            }
                        });
                    }
                    Fields::Unit =>
                    {
                        match_arms.push(quote!
                        {
                            Self::#variant_ident =>
                            {
                                ui.label("Variant");
                                ui.label(#variant_name_str);
                                ui.end_row();
                            }
                        });
                    }
                }
            }

            quote!
            {
                ::egui::Grid::new(#grid_id_str)
                    .striped(true)
                    .show(ui, |ui|
                    {
                        match self
                        {
                            #(#match_arms)*
                        }
                    });
            }
        }
        Data::Union(_) => panic!("DebugGui cannot be derived for unions"),
    };

    let display_name_str = type_name.to_string();

    let expanded = quote!
    {
        impl #impl_generics #debug_gui_trait for #type_name #ty_generics #where_clause
        {
            fn display_name(&self) -> &str { #display_name_str }
            fn debug_gui(&self, ui: &mut ::egui::Ui)
            {
                #debug_gui_body
            }
        }
    };

    TokenStream::from(expanded)
}