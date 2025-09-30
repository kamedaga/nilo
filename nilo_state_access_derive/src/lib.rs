use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument, LitStr,
};

#[proc_macro_derive(StateAccess, attributes(state_access))]
pub fn derive_state_access(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_ident = input.ident;

    // #[state_access(trait_path="::nilo::engine::state::StateAccess")]
    let mut trait_path_ts = quote!(::nilo::engine::state::StateAccess);
    for attr in &input.attrs {
        if !attr.path().is_ident("state_access") { continue; }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("trait_path") {
                let s: LitStr = meta.value()?.parse()?;
                let ts: proc_macro2::TokenStream =
                    s.value().parse().expect("invalid trait_path");
                trait_path_ts = quote!(#ts);
            }
            Ok(())
        });
    }

    let Data::Struct(data_struct) = &input.data else {
        return quote! { compile_error!("StateAccess can only be derived for structs"); }.into();
    };
    let Fields::Named(fields_named) = &data_struct.fields else {
        return quote! { compile_error!("StateAccess requires named fields"); }.into();
    };

    struct F { ident: syn::Ident, key: String, ty: Type }
    fn field_key(f: &syn::Field) -> String {
        // #[state_access(rename="alias")]
        let mut key = f.ident.as_ref().unwrap().to_string();
        for a in &f.attrs {
            if !a.path().is_ident("state_access") { continue; }
            let _ = a.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let s: LitStr = meta.value()?.parse()?;
                    key = s.value();
                }
                Ok(())
            });
        }
        key
    }

    let mut fs: Vec<F> = Vec::new();
    for f in &fields_named.named {
        fs.push(F {
            ident: f.ident.clone().unwrap(),
            key: field_key(f),
            ty: f.ty.clone(),
        });
    }

    // ---- 型ヘルパ ----
    fn is_ty(ty: &Type, want: &str) -> bool {
        let actual_type = match ty {
            Type::Group(group) => &*group.elem,  // Type::Groupの場合は内部の型を取得
            other => other,
        };

        match actual_type {
            Type::Path(tp) => {
                if let Some(seg) = tp.path.segments.last() {
                    return seg.ident == want;
                }
                false
            }
            _ => false,
        }
    }
    fn vec_inner(ty: &Type) -> Option<Type> {
        let actual_type = match ty {
            Type::Group(group) => &*group.elem,  // Type::Groupの場合は内部の型を取得
            other => other,
        };

        if let Type::Path(tp) = actual_type {
            if let Some(seg) = tp.path.segments.last() {
                if seg.ident == "Vec" {
                    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                        if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                            return Some(inner.clone());
                        }
                    }
                }
            }
        }
        None
    }

    // ---- get_field ----
    let get_field_arms = fs.iter().map(|f| {
        let name = &f.ident; let key = &f.key;
        if vec_inner(&f.ty).is_some() {
            // ベクター型の場合はJSONシリアライゼーションを使用
            quote! { #key => Some(serde_json::to_string(&self.#name).unwrap_or_else(|_| "[]".to_string())) }
        } else {
            // その他の型は通常のto_string()を使用
            quote! { #key => Some(self.#name.to_string()) }
        }
    });

    // ---- set ----
    let set_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if is_ty(&f.ty, "String") {
            quote! { #key => { self.#field = value; Ok(()) } }
        } else if is_ty(&f.ty, "bool") {
            quote! { #key => {
                let v = matches!(value.as_str(), "true"|"1"|"True"|"TRUE");
                self.#field = v; Ok(())
            } }
        } else if is_ty(&f.ty, "i8")||is_ty(&f.ty, "i16")||is_ty(&f.ty, "i32")||is_ty(&f.ty, "i64")||is_ty(&f.ty, "i128")
            ||is_ty(&f.ty, "u8")||is_ty(&f.ty, "u16")||is_ty(&f.ty, "u32")||is_ty(&f.ty, "u64")||is_ty(&f.ty, "u128") {
            let ty = &f.ty;
            quote! { #key => {
                self.#field = value.parse::<#ty>().map_err(|e| format!("parse {}: {}", #key, e))?;
                Ok(())
            } }
        } else if is_ty(&f.ty, "f32")||is_ty(&f.ty, "f64") {
            let ty = &f.ty;
            quote! { #key => { self.#field = value.parse::<#ty>().map_err(|e| format!("parse {}: {}", #key, e))?; Ok(()) } }
        } else if vec_inner(&f.ty).is_some() {
            quote! { #key => { Err(format!("{} is a list; use list_append/remove", #key)) } }
        } else {
            quote! { #key => {
                Err(format!("unsupported type for set: {}", #key))
            } }
        }
    });

    // ---- toggle ----
    let toggle_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if is_ty(&f.ty, "bool") {
            quote! { #key => { self.#field = !self.#field; Ok(()) } }
        } else {
            quote! { #key => { Err(format!("{} is not a bool", #key)) } }
        }
    });

    // ---- list_append ----
    let list_append_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if let Some(inner) = vec_inner(&f.ty) {
            if is_ty(&inner, "String") {
                quote! { #key => { self.#field.push(value); Ok(()) } }
            } else if is_ty(&inner, "bool") {
                quote! { #key => {
                    self.#field.push(matches!(value.as_str(),"true"|"1"|"True"|"TRUE"));
                    Ok(())
                } }
            } else {
                quote! { #key => {
                    let v = value.parse::<#inner>().map_err(|e| format!("parse {} item: {}", #key, e))?;
                    self.#field.push(v); Ok(())
                } }
            }
        } else {
            quote! { #key => { Err(format!("{} is not a list", #key)) } }
        }
    });

    // ---- list_remove ----
    let list_remove_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if vec_inner(&f.ty).is_some() {
            quote! { #key => {
                if index < self.#field.len() { self.#field.remove(index); }
                Ok(())
            } }
        } else {
            quote! { #key => { Err(format!("{} is not a list", #key)) } }
        }
    });

    // ---- list_clear ----
    let list_clear_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if vec_inner(&f.ty).is_some() {
            quote! { #key => {
                self.#field.clear();
                Ok(())
            } }
        } else {
            quote! { #key => { Err(format!("{} is not a list", #key)) } }
        }
    });

    let expanded = quote! {
        impl #trait_path_ts for #struct_ident {
            fn get_field(&self, key: &str) -> Option<String> {
                match key { #(#get_field_arms,)* _ => None }
            }
            fn set(&mut self, path: &str, value: String) -> Result<(), String> {
                match path { #(#set_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
            fn toggle(&mut self, path: &str) -> Result<(), String> {
                match path { #(#toggle_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
            fn list_append(&mut self, path: &str, value: String) -> Result<(), String> {
                match path { #(#list_append_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
            fn list_remove(&mut self, path: &str, index: usize) -> Result<(), String> {
                match path { #(#list_remove_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
            fn list_clear(&mut self, path: &str) -> Result<(), String> {
                match path { #(#list_clear_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
        }
    };
    TokenStream::from(expanded)
}
