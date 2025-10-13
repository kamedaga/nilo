use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Error, Expr, ExprLit, Fields, FnArg, GenericArgument, Lit,
    LitStr, Meta, PathArguments, Type, TypePath,
};
use syn::punctuated::Punctuated;

// Top-level type inspection helpers for attribute macros
fn is_ty(ty: &Type, want: &str) -> bool {
    let actual_type = match ty {
        Type::Group(group) => &*group.elem,
        other => other,
    };
    match actual_type {
        Type::Path(tp) => tp.path.segments.last().map(|seg| seg.ident == want).unwrap_or(false),
        Type::Reference(r) => is_ty(&r.elem, want),
        _ => false,
    }
}

fn sanitize_identifier(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_uppercase());
        } else {
            result.push('_');
        }
    }
    if result.is_empty() {
        result.push('_');
    }
    if result.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        result.insert(0, '_');
    }
    result
}

fn parse_state_type(expr: &Expr) -> Result<Type, Error> {
    match expr {
        Expr::Path(path) => Ok(Type::Path(TypePath {
            qself: path.qself.clone(),
            path: path.path.clone(),
        })),
        Expr::Lit(ExprLit { lit: Lit::Str(lit_str), .. }) => {
            syn::parse_str::<Type>(&lit_str.value()).map_err(|e| Error::new(lit_str.span(), e))
        }
        other => Err(Error::new_spanned(other, "state must be a type path or string literal")),
    }
}

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
            quote! { #key => { self.#field = value; ::nilo::engine::state::notify_state_watchers(self, #key); Ok(()) } }
        } else if is_ty(&f.ty, "bool") {
            quote! { #key => {
                let v = matches!(value.as_str(), "true"|"1"|"True"|"TRUE");
                self.#field = v;
                ::nilo::engine::state::notify_state_watchers(self, #key);
                Ok(())
            } }
        } else if is_ty(&f.ty, "i8")||is_ty(&f.ty, "i16")||is_ty(&f.ty, "i32")||is_ty(&f.ty, "i64")||is_ty(&f.ty, "i128")
            ||is_ty(&f.ty, "u8")||is_ty(&f.ty, "u16")||is_ty(&f.ty, "u32")||is_ty(&f.ty, "u64")||is_ty(&f.ty, "u128") {
            let ty = &f.ty;
            quote! { #key => {
                self.#field = value.parse::<#ty>().map_err(|e| format!("parse {}: {}", #key, e))?;
                ::nilo::engine::state::notify_state_watchers(self, #key);
                Ok(())
            } }
        } else if is_ty(&f.ty, "f32")||is_ty(&f.ty, "f64") {
            let ty = &f.ty;
            quote! { #key => {
                self.#field = value.parse::<#ty>().map_err(|e| format!("parse {}: {}", #key, e))?;
                ::nilo::engine::state::notify_state_watchers(self, #key);
                Ok(())
            } }
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
            quote! { #key => {
                self.#field = !self.#field;
                ::nilo::engine::state::notify_state_watchers(self, #key);
                Ok(())
            } }
        } else {
            quote! { #key => { Err(format!("{} is not a bool", #key)) } }
        }
    });

    // ---- list_append ----
    let list_append_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if let Some(inner) = vec_inner(&f.ty) {
            if is_ty(&inner, "String") {
                quote! { #key => {
                    self.#field.push(value);
                    ::nilo::engine::state::notify_state_watchers(self, #key);
                    Ok(())
                } }
            } else if is_ty(&inner, "bool") {
                quote! { #key => {
                    self.#field.push(matches!(value.as_str(),"true"|"1"|"True"|"TRUE"));
                    ::nilo::engine::state::notify_state_watchers(self, #key);
                    Ok(())
                } }
            } else {
                quote! { #key => {
                    let v = value.parse::<#inner>().map_err(|e| format!("parse {} item: {}", #key, e))?;
                    self.#field.push(v);
                    ::nilo::engine::state::notify_state_watchers(self, #key);
                    Ok(())
                } }
            }
        } else {
            quote! { #key => { Err(format!("{} is not a list", #key)) } }
        }
    });

    // ---- list_insert ----
    let list_insert_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if let Some(inner) = vec_inner(&f.ty) {
            if is_ty(&inner, "String") {
                quote! { #key => { 
                    if index <= self.#field.len() {
                        self.#field.insert(index, value); 
                        ::nilo::engine::state::notify_state_watchers(self, #key);
                        Ok(())
                    } else {
                        Err("Index out of bounds".to_string())
                    }
                } }
            } else if is_ty(&inner, "bool") {
                quote! { #key => {
                    if index <= self.#field.len() {
                        let v = matches!(value.as_str(),"true"|"1"|"True"|"TRUE");
                        self.#field.insert(index, v);
                        ::nilo::engine::state::notify_state_watchers(self, #key);
                        Ok(())
                    } else {
                        Err("Index out of bounds".to_string())
                    }
                } }
            } else {
                quote! { #key => {
                    if index <= self.#field.len() {
                        let v = value.parse::<#inner>().map_err(|e| format!("parse {} item: {}", #key, e))?;
                        self.#field.insert(index, v);
                        ::nilo::engine::state::notify_state_watchers(self, #key);
                        Ok(())
                    } else {
                        Err("Index out of bounds".to_string())
                    }
                } }
            }
        } else {
            quote! { #key => { Err(format!("{} is not a list", #key)) } }
        }
    });

    // ---- list_remove ----
    let list_remove_arms = fs.iter().map(|f| {
        let field = &f.ident; let key = &f.key;
        if let Some(inner) = vec_inner(&f.ty) {
            if is_ty(&inner, "String") {
                quote! { #key => {
                    if let Some(pos) = self.#field.iter().position(|x| *x == value) {
                        self.#field.remove(pos);
                        ::nilo::engine::state::notify_state_watchers(self, #key);
                        Ok(())
                    } else {
                        Err(format!("Item {} not found", value))
                    }
                } }
            } else if is_ty(&inner, "bool") {
                quote! { #key => {
                    let target = matches!(value.as_str(),"true"|"1"|"True"|"TRUE");
                    if let Some(pos) = self.#field.iter().position(|x| *x == target) {
                        self.#field.remove(pos);
                        ::nilo::engine::state::notify_state_watchers(self, #key);
                        Ok(())
                    } else {
                        Err(format!("Item {} not found", value))
                    }
                } }
            } else {
                quote! { #key => {
                    let v = value.parse::<#inner>().map_err(|e| format!("parse {} item: {}", #key, e))?;
                    if let Some(pos) = self.#field.iter().position(|x| *x == v) {
                        self.#field.remove(pos);
                        ::nilo::engine::state::notify_state_watchers(self, #key);
                        Ok(())
                    } else {
                        Err(format!("Item {} not found", value))
                    }
                } }
            }
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
                ::nilo::engine::state::notify_state_watchers(self, #key);
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
            fn list_insert(&mut self, path: &str, index: usize, value: String) -> Result<(), String> {
                match path { #(#list_insert_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
            fn list_remove(&mut self, path: &str, value: String) -> Result<(), String> {
                match path { #(#list_remove_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
            fn list_clear(&mut self, path: &str) -> Result<(), String> {
                match path { #(#list_clear_arms,)* _ => Err(format!("unknown field: {}", path)) }
            }
        }
    };
    TokenStream::from(expanded)
}

// ========================================
// Nilo関数登録マクロ
// ========================================

/// Nilo関数を自動登録する属性マクロ
/// 
/// # 使用例
/// ```rust
/// #[nilo_function]
/// fn greet() {
///     println!("Hello!");
/// }
/// 
/// #[nilo_function]
/// fn open_url(url: String) {
///     println!("Opening: {}", url);
/// }
/// 
/// #[nilo_function]
/// fn add(a: i32, b: i32) {
///     println!("{} + {} = {}", a, b, a + b);
/// }
/// ```
#[proc_macro_attribute]
pub fn nilo_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;
    let attrs = &input.attrs;
    
    // 関数をそのまま保持しつつ、登録用のコードも生成
    let register_ident = quote::format_ident!("__NILO_REGISTER_{}", fn_name.to_string().to_uppercase());
    
    let expanded = quote! {
        // 元の関数をそのまま保持
        #(#attrs)*
        #vis #sig #block
        
        // 自動登録用のスタティック変数を生成（WASM以外のみ）
        #[cfg(not(target_arch = "wasm32"))]
        #[::linkme::distributed_slice(::nilo::NILO_FUNCTION_REGISTRY)]
        #[allow(non_upper_case_globals)]
        static #register_ident: fn() = || {
            ::nilo::register_typed_call(#fn_name_str, #fn_name);
        };
    };
    
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn nilo_state_watcher(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as syn::ItemFn);

    let mut state_ty: Option<Type> = None;
    let mut fields: Vec<LitStr> = Vec::new();
    let mut errors: Vec<Error> = Vec::new();

    for arg in args {
        match arg {
            Meta::NameValue(nv) => {
                let Some(ident) = nv.path.get_ident() else {
                    errors.push(Error::new_spanned(&nv.path, "invalid attribute key"));
                    continue;
                };
                match ident.to_string().as_str() {
                    "state" => match parse_state_type(&nv.value) {
                        Ok(ty) => state_ty = Some(ty),
                        Err(e) => errors.push(e),
                    },
                    "field" => match &nv.value {
                        Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => fields.push(lit.clone()),
                        other => errors.push(Error::new_spanned(other, "field must be a string literal")),
                    },
                    _ => errors.push(Error::new_spanned(ident, "unsupported attribute key")),
                }
            }
            Meta::List(list) if list.path.is_ident("fields") => {
                match list.parse_args_with(Punctuated::<LitStr, syn::Token![,]>::parse_terminated) {
                    Ok(lits) => fields.extend(lits.into_iter()),
                    Err(e) => errors.push(e),
                }
            }
            other => errors.push(Error::new_spanned(other, "unsupported attribute argument")),
        }
    }

    if let Some(err) = errors.into_iter().reduce(|mut acc, err| {
        acc.combine(err);
        acc
    }) {
        return err.to_compile_error().into();
    }

    let Some(state_ty) = state_ty else {
        return Error::new(Span::call_site(), "state attribute is required").to_compile_error().into();
    };

    if fields.is_empty() {
        return Error::new(Span::call_site(), "at least one field must be specified").to_compile_error().into();
    }

    if input.sig.inputs.len() != 1 {
        return Error::new_spanned(
            &input.sig.inputs,
            "nilo_state_watcher functions must take exactly one argument: &mut State",
        )
        .to_compile_error()
        .into();
    }

    let arg_ok = match input.sig.inputs.first().unwrap() {
        FnArg::Receiver(_) => false,
        FnArg::Typed(pat_ty) => match pat_ty.ty.as_ref() {
            Type::Reference(reference) => reference.mutability.is_some(),
            _ => false,
        },
    };

    if !arg_ok {
        return Error::new_spanned(
            &input.sig.inputs,
            "nilo_state_watcher argument must be &mut <State>",
        )
        .to_compile_error()
        .into();
    }

    let fn_name = &input.sig.ident;
    let fn_name_sanitized = sanitize_identifier(&fn_name.to_string());

    let register_statics = fields.iter().enumerate().map(|(idx, lit)| {
        let field_sanitized = sanitize_identifier(&lit.value());
        let static_ident = quote::format_ident!(
            "__NILO_STATE_WATCHER_REGISTER_{}_{}_{}",
            fn_name_sanitized,
            field_sanitized,
            idx
        );
        quote! {
            #[cfg(not(target_arch = "wasm32"))]
            #[::linkme::distributed_slice(::nilo::engine::state::STATE_WATCHER_BOOTSTRAP)]
            #[allow(non_upper_case_globals)]
            static #static_ident: fn() = || {
                ::nilo::engine::state::register_state_watcher::<#state_ty, _>(#lit, #fn_name);
            };
        }
    });

    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    TokenStream::from(quote! {
        #(#attrs)*
        #vis #sig #block

        #(#register_statics)*
    })
}

/// Attribute macro to generate an assign helper that writes a Rust value into Nilo state.
///
/// Usage:
/// - #[nilo_state_assign(state = MyState, field = "counter")]
///   fn set_counter(state: &mut MyState, value: i32) -> Result<(), String> { /* body ignored */ }
///
/// The macro validates the signature and generates a body that calls StateAccess::set
/// with proper type conversion for Nilo-supported scalar types (String, bool, integers, floats).
#[proc_macro_attribute]
pub fn nilo_state_assign(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as syn::ItemFn);

    // Parse attr: state = <Type>, field = "name"
    let mut state_ty: Option<Type> = None;
    let mut field: Option<LitStr> = None;
    let mut errors: Vec<Error> = Vec::new();

    for arg in args {
        match arg {
            Meta::NameValue(nv) => {
                let Some(ident) = nv.path.get_ident() else {
                    errors.push(Error::new_spanned(&nv.path, "invalid attribute key"));
                    continue;
                };
                match ident.to_string().as_str() {
                    "state" => match parse_state_type(&nv.value) {
                        Ok(ty) => state_ty = Some(ty),
                        Err(e) => errors.push(e),
                    },
                    "field" => match &nv.value {
                        Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => field = Some(lit.clone()),
                        other => errors.push(Error::new_spanned(other, "field must be a string literal")),
                    },
                    _ => errors.push(Error::new_spanned(ident, "unsupported attribute key")),
                }
            }
            other => errors.push(Error::new_spanned(other, "unsupported attribute argument")),
        }
    }

    if let Some(err) = errors.into_iter().reduce(|mut acc, err| { acc.combine(err); acc }) {
        return err.to_compile_error().into();
    }

    let state_ty = match state_ty {
        Some(t) => t,
        None => return Error::new(Span::call_site(), "state attribute is required").to_compile_error().into(),
    };
    let field = match field {
        Some(f) => f,
        None => return Error::new(Span::call_site(), "field attribute is required").to_compile_error().into(),
    };

    // Validate function signature: (&mut State, T) -> Result<(), String>
    if input.sig.inputs.len() != 2 {
        return Error::new_spanned(
            &input.sig.inputs,
            "nilo_state_assign function must take exactly two args: (&mut State, value)",
        )
        .to_compile_error()
        .into();
    }

    // First arg must be &mut state_ty
    let mut value_ty_opt: Option<Type> = None;
    let mut first_ok = false;
    if let (Some(first), Some(second)) = (input.sig.inputs.first(), input.sig.inputs.iter().nth(1)) {
        // First
        if let FnArg::Typed(pat_ty) = first {
            if let Type::Reference(rf) = pat_ty.ty.as_ref() {
                if rf.mutability.is_some() {
                    let arg_ty = quote!(#rf.elem).to_string();
                    let state_ty_tokens = quote!(#state_ty).to_string();
                    first_ok = arg_ty == state_ty_tokens;
                }
            }
        }
        // Second: capture value type
        if let FnArg::Typed(pat_ty) = second {
            value_ty_opt = Some((*pat_ty.ty).clone());
        }
    }
    if !first_ok {
        return Error::new_spanned(&input.sig.inputs, "first argument must be &mut <State>")
            .to_compile_error()
            .into();
    }
    let Some(value_ty) = value_ty_opt else {
        return Error::new_spanned(&input.sig.inputs, "missing value argument").to_compile_error().into();
    };

    // Return type must be Result<(), String>
    let returns_result = match &input.sig.output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => {
            if let Type::Path(tp) = ty.as_ref() {
                if let Some(seg) = tp.path.segments.last() {
                    if seg.ident == "Result" {
                        // Cheap check on generic args
                        true
                    } else { false }
                } else { false }
            } else { false }
        }
    };
    if !returns_result {
        return Error::new_spanned(&input.sig.output, "return type must be Result<(), String>")
            .to_compile_error()
            .into();
    }

    // Build conversion from value_ty -> String
    let value_to_string = {
        if is_ty(&value_ty, "String") {
            quote! { value }
        } else if let Type::Reference(rf) = &value_ty {
            // &str supported
            if is_ty(&rf.elem, "str") {
                quote! { value.to_string() }
            } else {
                quote! { value.to_string() }
            }
        } else if is_ty(&value_ty, "bool") {
            quote! { if value { "true".to_string() } else { "false".to_string() } }
        } else if is_ty(&value_ty, "i8")||is_ty(&value_ty, "i16")||is_ty(&value_ty, "i32")||is_ty(&value_ty, "i64")||is_ty(&value_ty, "i128")
            ||is_ty(&value_ty, "u8")||is_ty(&value_ty, "u16")||is_ty(&value_ty, "u32")||is_ty(&value_ty, "u64")||is_ty(&value_ty, "u128")
            ||is_ty(&value_ty, "f32")||is_ty(&value_ty, "f64") {
            quote! { value.to_string() }
        } else {
            return Error::new_spanned(
                &value_ty,
                "unsupported value type for nilo_state_assign (allowed: String, &str, bool, integers, floats)",
            )
            .to_compile_error()
            .into();
        }
    };

    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &input.sig;

    let expanded = quote! {
        #(#attrs)*
        #vis #sig {
            let result = ::nilo::engine::state::StateAccess::set(
                state,
                #field,
                { let value = value; #value_to_string }
            );
            result
        }
    };

    TokenStream::from(expanded)
}

/// Attribute macro to validate a Nilo state field with a Rust predicate function.
///
/// Usage:
/// - #[nilo_state_validator(state = MyState, field = "age")]
///   fn validate_age(v: i32) -> bool { v >= 0 && v <= 120 }
///
/// Registers a watcher that parses the field into the function's parameter type and
/// calls the function. If it returns false, an error is logged.
#[proc_macro_attribute]
pub fn nilo_state_validator(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as syn::ItemFn);

    // Parse attr: state = <Type>, field = "name"
    let mut state_ty: Option<Type> = None;
    let mut field: Option<LitStr> = None;
    let mut errors: Vec<Error> = Vec::new();

    for arg in args {
        match arg {
            Meta::NameValue(nv) => {
                let Some(ident) = nv.path.get_ident() else {
                    errors.push(Error::new_spanned(&nv.path, "invalid attribute key"));
                    continue;
                };
                match ident.to_string().as_str() {
                    "state" => match parse_state_type(&nv.value) {
                        Ok(ty) => state_ty = Some(ty),
                        Err(e) => errors.push(e),
                    },
                    "field" => match &nv.value {
                        Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => field = Some(lit.clone()),
                        other => errors.push(Error::new_spanned(other, "field must be a string literal")),
                    },
                    _ => errors.push(Error::new_spanned(ident, "unsupported attribute key")),
                }
            }
            other => errors.push(Error::new_spanned(other, "unsupported attribute argument")),
        }
    }

    if let Some(err) = errors.into_iter().reduce(|mut acc, err| { acc.combine(err); acc }) {
        return err.to_compile_error().into();
    }
    let state_ty = match state_ty {
        Some(t) => t,
        None => return Error::new(Span::call_site(), "state attribute is required").to_compile_error().into(),
    };
    let field = match field {
        Some(f) => f,
        None => return Error::new(Span::call_site(), "field attribute is required").to_compile_error().into(),
    };

    // Validate validator function signature: fn(value: T) -> bool | Result<(), String>
    if input.sig.inputs.len() != 1 {
        return Error::new_spanned(&input.sig.inputs, "validator must take exactly one argument: value")
            .to_compile_error()
            .into();
    }
    let value_ty = if let FnArg::Typed(pat_ty) = input.sig.inputs.first().unwrap() {
        (*pat_ty.ty).clone()
    } else {
        return Error::new_spanned(&input.sig.inputs, "invalid argument list").to_compile_error().into();
    };

    let returns_bool_or_result = match &input.sig.output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Path(tp) => {
                if let Some(seg) = tp.path.segments.last() {
                    let id = seg.ident.to_string();
                    id == "bool" || id == "Result"
                } else { false }
            }
            _ => false,
        },
    };
    if !returns_bool_or_result {
        return Error::new_spanned(&input.sig.output, "return type must be bool or Result<(), String>")
            .to_compile_error()
            .into();
    }

    // Build parse from String -> value_ty
    let parse_value = {
        if is_ty(&value_ty, "String") {
            quote! { v_str }
        } else if is_ty(&value_ty, "bool") {
            quote! { matches!(v_str.as_str(), "true"|"1"|"True"|"TRUE") }
        } else if is_ty(&value_ty, "i8")||is_ty(&value_ty, "i16")||is_ty(&value_ty, "i32")||is_ty(&value_ty, "i64")||is_ty(&value_ty, "i128")
            ||is_ty(&value_ty, "u8")||is_ty(&value_ty, "u16")||is_ty(&value_ty, "u32")||is_ty(&value_ty, "u64")||is_ty(&value_ty, "u128")
            ||is_ty(&value_ty, "f32")||is_ty(&value_ty, "f64") {
            quote! { match v_str.parse::<#value_ty>() { Ok(v) => v, Err(_) => { log::error!("Failed to parse '{}' for validation", #field); return; } } }
        } else {
            return Error::new_spanned(&value_ty, "unsupported validator type; only scalar types supported").to_compile_error().into();
        }
    };

    let fn_name = &input.sig.ident;
    let fn_name_sanitized = sanitize_identifier(&fn_name.to_string());
    let register_ident = quote::format_ident!("__NILO_STATE_VALIDATOR_REGISTER_{}_{}", fn_name_sanitized, sanitize_identifier(&field.value()));

    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    // Rebuild per-return-type registration code
    let expanded = {
        let (bool_branch, result_branch) = match &input.sig.output {
            syn::ReturnType::Type(_, ty) => match ty.as_ref() {
                Type::Path(tp) if tp.path.segments.last().map(|s| s.ident == "bool").unwrap_or(false) => {
                    let reg = quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #[::linkme::distributed_slice(::nilo::engine::state::STATE_WATCHER_BOOTSTRAP)]
                        #[allow(non_upper_case_globals)]
                        static #register_ident: fn() = || {
                            ::nilo::engine::state::register_state_watcher(#field, |state: &mut #state_ty| {
                                if let Some(v_str) = ::nilo::engine::state::StateAccess::get_field(state, #field) {
                                    let value: #value_ty = { let v_str = v_str; #parse_value };
                                    if !#fn_name(value) {
                                        log::error!("Validation failed for {}::{}", std::any::type_name::<#state_ty>(), #field);
                                    }
                                }
                            });
                        };
                    };
                    (reg, quote! {})
                }
                Type::Path(tp) if tp.path.segments.last().map(|s| s.ident == "Result").unwrap_or(false) => {
                    let reg = quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #[::linkme::distributed_slice(::nilo::engine::state::STATE_WATCHER_BOOTSTRAP)]
                        #[allow(non_upper_case_globals)]
                        static #register_ident: fn() = || {
                            ::nilo::engine::state::register_state_watcher(#field, |state: &mut #state_ty| {
                                if let Some(v_str) = ::nilo::engine::state::StateAccess::get_field(state, #field) {
                                    let value: #value_ty = { let v_str = v_str; #parse_value };
                                    if let Err(e) = #fn_name(value) {
                                        log::error!("Validation failed for {}::{}: {}", std::any::type_name::<#state_ty>(), #field, e);
                                    }
                                }
                            });
                        };
                    };
                    (quote! {}, reg)
                }
                _ => (quote! {}, quote! {}),
            },
            _ => (quote! {}, quote! {}),
        };

        quote! {
            #(#attrs)*
            #vis #sig #block

            #bool_branch
            #result_branch
        }
    };

    TokenStream::from(expanded)
}

/// Attribute to auto-register a state-accessible onclick function.
///
/// ⚠️ **重要な注意事項**:
/// このマクロは `AppState` 全体へのアクセスが必要な関数専用です。
/// 単にカスタムステートのみにアクセスする場合は、
/// `register_safe_state_call` を使用することを強く推奨します。
///
/// # いつ使うべきか
///
/// - `AppState` の他のフィールド（`current_timeline`, `variables` など）にアクセスする必要がある場合
/// - エンジンの内部状態を直接操作する必要がある場合（上級者向け）
///
/// # いつ使うべきでないか
///
/// - カスタムステートのフィールドの読み書きだけで十分な場合
/// - 軽量で安全な実装が望ましい場合
///
/// # 推奨される代替案
///
/// ```rust
/// // ✅ 推奨: register_safe_state_call を使用
/// register_safe_state_call("increment_counter", |ctx: &mut CustomStateContext<State>, _args| {
///     if let Some(current) = ctx.get_as::<i32>("counter") {
///         let _ = ctx.set("counter", (current + 1).to_string());
///     }
/// });
/// ```
///
/// # このマクロの使用例
///
/// ```rust
/// // ⚠️ AppState全体が必要な場合のみ
/// #[nilo_state_accessible(state = MyState, name = "advanced_function")]
/// fn advanced_function<S>(state: &mut AppState<S>, args: &[Expr])
/// where S: StateAccess
/// {
///     // state.current_timeline, state.variables などにアクセス可能
///     log::info!("Current timeline: {}", state.current_timeline);
///     // カスタムステートにもアクセス可能
///     let _ = state.custom_state.set("field", "value".to_string());
/// }
/// ```
///
/// Usage:
/// #[nilo_state_accessible(state = MyState, name = "increment_counter")]
/// fn increment<S>(state: &mut ::nilo::AppState<S>, args: &[::nilo::parser::ast::Expr]) where S: ::nilo::StateAccess { ... }
#[proc_macro_attribute]
pub fn nilo_state_accessible(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as syn::ItemFn);

    let mut state_ty: Option<Type> = None;
    let mut name: Option<LitStr> = None;
    let mut errors: Vec<Error> = Vec::new();

    for arg in args {
        match arg {
            Meta::NameValue(nv) => {
                let Some(ident) = nv.path.get_ident() else {
                    errors.push(Error::new_spanned(&nv.path, "invalid attribute key"));
                    continue;
                };
                match ident.to_string().as_str() {
                    "state" => match parse_state_type(&nv.value) {
                        Ok(ty) => state_ty = Some(ty),
                        Err(e) => errors.push(e),
                    },
                    "name" => match &nv.value {
                        Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => name = Some(lit.clone()),
                        other => errors.push(Error::new_spanned(other, "name must be a string literal")),
                    },
                    _ => errors.push(Error::new_spanned(ident, "unsupported attribute key")),
                }
            }
            other => errors.push(Error::new_spanned(other, "unsupported attribute argument")),
        }
    }

    if let Some(err) = errors.into_iter().reduce(|mut acc, err| { acc.combine(err); acc }) {
        return err.to_compile_error().into();
    }

    let state_ty = match state_ty {
        Some(t) => t,
        None => return Error::new(Span::call_site(), "state attribute is required").to_compile_error().into(),
    };
    let name_str = name.map(|s| s.value()).unwrap_or_else(|| input.sig.ident.to_string());

    let fn_ident = &input.sig.ident;
    let register_ident = quote::format_ident!("__NILO_STATE_ACCESSIBLE_REGISTER_{}", sanitize_identifier(&name_str));

    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    let expanded = quote! {
        #(#attrs)*
        #vis #sig #block

        #[cfg(not(target_arch = "wasm32"))]
        #[::linkme::distributed_slice(::nilo::engine::rust_call::STATE_ACCESSIBLE_BOOTSTRAP)]
        #[allow(non_upper_case_globals)]
        static #register_ident: fn() = || {
            // 従来の register_state_accessible_call を使用
            // マクロの利便性を優先して、内部的には非推奨APIを使用
            #[allow(deprecated)]
            ::nilo::register_state_accessible_call(#name_str, #fn_ident::<#state_ty>);
        };
    };

    TokenStream::from(expanded)
}

/// Attribute to auto-register a safe (no AppState access) onclick function.
///
/// Usage (user function receives a limited CustomStateContext):
/// #[nilo_safe_accessible(state = MyState, name = "set_name")]
/// fn set_name(ctx: &mut ::nilo::CustomStateContext<MyState>, args: &[::nilo::parser::ast::Expr]) { /* ... */ }
///
/// The macro generates an adapter specialized to `state = T` that converts
/// `&mut AppState<T>` into `CustomStateContext<T>` and registers that adapter
/// for onclick via the state-accessible registry.
#[proc_macro_attribute]
pub fn nilo_safe_accessible(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as syn::ItemFn);

    let mut state_ty: Option<Type> = None;
    let mut name: Option<LitStr> = None;
    let mut errors: Vec<Error> = Vec::new();

    for arg in args {
        match arg {
            Meta::NameValue(nv) => {
                let Some(ident) = nv.path.get_ident() else {
                    errors.push(Error::new_spanned(&nv.path, "invalid attribute key"));
                    continue;
                };
                match ident.to_string().as_str() {
                    "state" => match parse_state_type(&nv.value) {
                        Ok(ty) => state_ty = Some(ty),
                        Err(e) => errors.push(e),
                    },
                    "name" => match &nv.value {
                        Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => name = Some(lit.clone()),
                        other => errors.push(Error::new_spanned(other, "name must be a string literal")),
                    },
                    _ => errors.push(Error::new_spanned(ident, "unsupported attribute key")),
                }
            }
            other => errors.push(Error::new_spanned(other, "unsupported attribute argument")),
        }
    }

    if let Some(err) = errors.into_iter().reduce(|mut acc, err| { acc.combine(err); acc }) {
        return err.to_compile_error().into();
    }

    let state_ty = match state_ty {
        Some(t) => t,
        None => return Error::new(Span::call_site(), "state attribute is required").to_compile_error().into(),
    };
    let name_str = name.map(|s| s.value()).unwrap_or_else(|| input.sig.ident.to_string());

    let fn_ident = &input.sig.ident;
    let register_ident = quote::format_ident!("__NILO_SAFE_ACCESSIBLE_REGISTER_{}", sanitize_identifier(&name_str));

    let attrs = &input.attrs;
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    // Generate: user fn as-is + register adapter into state-accessible registry
    let expanded = quote! {
        #(#attrs)*
        #vis #sig #block

        #[cfg(not(target_arch = "wasm32"))]
        #[::linkme::distributed_slice(::nilo::engine::rust_call::STATE_ACCESSIBLE_BOOTSTRAP)]
        #[allow(non_upper_case_globals)]
        static #register_ident: fn() = || {
            // Register adapter: (&mut AppState<State>, &[Expr]) -> user(ctx, args)
            fn __nilo_safe_accessible_adapter(app_state: &mut ::nilo::AppState<#state_ty>, args: &[::nilo::parser::ast::Expr]) {
                ::nilo::engine::state::with_custom_state(app_state, |ctx| {
                    #fn_ident(ctx, args);
                });
            }
            #[allow(deprecated)]
            ::nilo::register_state_accessible_call(#name_str, __nilo_safe_accessible_adapter);
        };
    };

    TokenStream::from(expanded)
}
