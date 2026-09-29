//! Fork from https://docs.rs/crate/rig-tool-macro/0.5.0

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Ident, LitStr, Meta, Result, Token};
use syn::{FnArg, ItemFn, PatType, ReturnType, Type, parse_macro_input};

#[derive(Debug, Default)]
struct ToolAttribute {
    name: Option<String>,
    description: Option<String>,
    args: Vec<ArgMeta>,
}

#[derive(Debug)]
struct ArgMeta {
    name: String,
    description: Option<String>,
    required: Option<bool>,
}

impl Parse for ToolAttribute {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut attr = ToolAttribute::default();

        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

        for meta in metas {
            match meta {
                Meta::NameValue(nv) => {
                    let ident = nv
                        .path
                        .get_ident()
                        .ok_or_else(|| Error::new_spanned(&nv.path, "Expected identifier"))?;

                    let value = nv.value.clone();
                    let lit_result = syn::parse2::<LitStr>(nv.value.into_token_stream());
                    match (ident.to_string().as_str(), lit_result) {
                        ("name", Ok(lit)) => attr.name = Some(lit.value()),
                        ("description", Ok(lit)) => attr.description = Some(lit.value()),
                        (_, Err(e)) => {
                            return Err(Error::new_spanned(
                                value,
                                format!("Expected string literal, error: {e}"),
                            ));
                        }
                        _ => {
                            return Err(Error::new_spanned(
                                ident,
                                format!("Unknown attribute key: {}", ident),
                            ));
                        }
                    }
                }

                Meta::List(list) if list.path.is_ident("arg") => {
                    let args =
                        list.parse_args_with(Punctuated::<ArgMeta, Token![,]>::parse_terminated)?;
                    attr.args.append(&mut args.into_iter().collect());
                }

                meta => {
                    return Err(Error::new_spanned(
                        meta,
                        "Unsupported attribute format, expected `key = value` or `arg(...)`",
                    ));
                }
            }
        }

        Ok(attr)
    }
}

impl Parse for ArgMeta {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut arg = ArgMeta {
            name: input.parse::<Ident>()?.to_string().trim().to_owned(),
            description: None,
            required: None,
        };

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

            for meta in metas {
                match meta {
                    Meta::NameValue(nv) => {
                        let ident = nv
                            .path
                            .get_ident()
                            .ok_or_else(|| Error::new_spanned(&nv.path, "Expected identifier"))?;

                        let value = nv.value.clone();
                        match ident.to_string().as_str() {
                            "description" => {
                                let lit = syn::parse2::<LitStr>(nv.value.into_token_stream())
                                    .map_err(|e| {
                                        Error::new_spanned(
                                            value,
                                            format!("Expected string literal for description, error: {}", e),
                                        )
                                    })?;
                                arg.description = Some(lit.value());
                            }
                            "required" => {
                                let lit = syn::parse2::<syn::LitBool>(nv.value.into_token_stream())
                                    .map_err(|e| {
                                        Error::new_spanned(
                                            value,
                                            format!("Expected boolean literal (true/false) for 'required' attribute, got: {}", e),
                                        )
                                    })?;
                                arg.required = Some(lit.value);
                            }
                            _ => {
                                return Err(Error::new_spanned(
                                    ident,
                                    format!("Unknown arg property: {}", ident),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(Error::new_spanned(
                            meta,
                            "Expected `key = value` format for arg properties",
                        ));
                    }
                }
            }
        }

        Ok(arg)
    }
}

/// PascalCase over any non-alphanumeric separator, so tool names like "get-weather"
/// still yield valid Rust identifiers.
fn to_pascal_case(s: &str) -> String {
    let pascal: String = s
        .split(|c: char| !c.is_alphanumeric())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect();
    if pascal.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{pascal}")
    } else {
        pascal
    }
}

/// Return the first generic type argument of `ty` if its last path segment is `wrapper`
/// (e.g. `Option<T>` or `std::option::Option<T>` → `T`).
fn generic_inner<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != wrapper {
        return None;
    }
    match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => match args.args.first() {
            Some(syn::GenericArgument::Type(inner)) => Some(inner),
            _ => None,
        },
        _ => None,
    }
}

fn is_option(ty: &Type) -> bool {
    generic_inner(ty, "Option").is_some()
}

fn get_json_type(ty: &Type) -> TokenStream2 {
    // Option<T> is described by T; optionality is expressed through `required`.
    if let Some(inner) = generic_inner(ty, "Option") {
        return get_json_type(inner);
    }
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last().expect("empty type path");
            let type_name = segment.ident.to_string();

            // Handle Vec types
            if type_name == "Vec" {
                if let Some(inner_type) = generic_inner(ty, "Vec") {
                    let inner_json_type = get_json_type(inner_type);
                    return quote! {
                        "type": "array",
                        "items": { #inner_json_type }
                    };
                }
                return quote! { "type": "array" };
            }

            // Handle primitive types
            match type_name.as_str() {
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
                | "u128" | "usize" => {
                    quote! { "type": "integer" }
                }
                "f32" | "f64" => {
                    quote! { "type": "number" }
                }
                "String" | "str" | "char" => {
                    quote! { "type": "string" }
                }
                "bool" => {
                    quote! { "type": "boolean" }
                }
                // Handle other types as objects
                _ => {
                    quote! { "type": "object" }
                }
            }
        }
        _ => quote! { "type": "object" },
    }
}

/// Check if the given type is a custom struct or Vec<Struct> (not a primitive or standard library type)
fn is_custom_struct(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last().expect("empty type path");
            let type_name = segment.ident.to_string();

            // Check if it's a Vec<T>
            if type_name == "Vec" {
                return generic_inner(ty, "Vec").is_some_and(is_custom_struct);
            }

            // List of known primitive and standard library types
            !matches!(
                type_name.as_str(),
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
                    | "bool"
                    | "char"
                    | "String"
                    | "str"
                    | "Vec"
                    | "Option"
                    | "Result"
            )
        }
        _ => false,
    }
}

/// Whether `ident` appears anywhere in `tokens`, including inside nested groups.
fn mentions_ident(tokens: TokenStream2, ident: &Ident) -> bool {
    tokens.into_iter().any(|tree| match tree {
        proc_macro2::TokenTree::Ident(found) => &found == ident,
        proc_macro2::TokenTree::Group(group) => mentions_ident(group.stream(), ident),
        _ => false,
    })
}

pub fn tool_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let tool_attr = parse_macro_input!(attr as ToolAttribute);
    let input_fn = parse_macro_input!(item as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let tool_name = match tool_attr.name {
        Some(name) => name,
        None => input_fn.sig.ident.unraw().to_string(),
    };
    // OpenAI and Anthropic reject any other tool name with a 400 on every request.
    let valid_name = !tool_name.is_empty()
        && tool_name.len() <= 64
        && tool_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid_name {
        return Error::new_spanned(
            &input_fn.sig.ident,
            format!(
                "tool name `{tool_name}` must be 1-64 characters of ASCII letters, digits, `_` or `-`; set a valid one with #[tool(name = \"...\")]"
            ),
        )
        .to_compile_error()
        .into();
    }

    let struct_name = quote::format_ident!("{}Tool", to_pascal_case(&tool_name));
    let static_name = quote::format_ident!("{}", to_pascal_case(&tool_name));

    // Extract return type: Result<T, E>, matched on the last path segment so
    // `std::result::Result<T, E>` works. The error type must be spelled out: aliases such
    // as `io::Result<T>` hide it from the macro.
    const RETURN_TYPE_ERROR: &str = "Function must return `Result<T, E>` with an explicit error type (type aliases such as `io::Result<T>` are not supported)";
    let (return_type, error_type) = if let ReturnType::Type(_, ty) = &input_fn.sig.output {
        let Type::Path(type_path) = ty.as_ref() else {
            panic!("{RETURN_TYPE_ERROR}");
        };
        let segment = type_path
            .path
            .segments
            .last()
            .expect("empty return type path");
        if segment.ident != "Result" {
            panic!("{RETURN_TYPE_ERROR}");
        }
        let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
            panic!("{RETURN_TYPE_ERROR}");
        };
        match args.args.iter().collect::<Vec<_>>().as_slice() {
            [syn::GenericArgument::Type(t), syn::GenericArgument::Type(e)] => (t, e.clone()),
            _ => panic!("{RETURN_TYPE_ERROR}"),
        }
    } else {
        panic!("Function must return a Result")
    };

    let args: Vec<(&Ident, &Type)> = input_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(PatType { pat, ty, .. }) => match pat.as_ref() {
                // Only the identifier is reused, so `mut x` becomes a plain `x` field.
                syn::Pat::Ident(pat_ident) => Some((&pat_ident.ident, ty.as_ref())),
                _ => panic!("Only simple identifiers are supported in tool arguments"),
            },
            FnArg::Receiver(_) => None,
        })
        .collect();

    let arg_names: Vec<_> = args.iter().map(|(ident, _)| *ident).collect();
    let arg_types: Vec<_> = args.iter().map(|(_, ty)| *ty).collect();
    // JSON property names: serde serializes the field `r#type` as `type`.
    let arg_json_names: Vec<_> = arg_names
        .iter()
        .map(|ident| ident.unraw().to_string())
        .collect();
    let json_types: Vec<_> = arg_types.iter().map(|ty| get_json_type(ty)).collect();

    let is_struct_args = arg_types.iter().any(|ty| is_custom_struct(ty));

    // if arg is struct,  it should be the only arg
    if is_struct_args && arg_types.len() != 1 {
        panic!("Struct args must be the only arg");
    }

    // Validate that optional arguments are Option types
    for arg in &tool_attr.args {
        if let Some(false) = arg.required {
            // Find the corresponding argument type
            if let Some((_, ty)) = args.iter().find(|(ident, _)| **ident == arg.name)
                && !is_option(ty)
            {
                panic!(
                    "Argument '{}' is marked as optional (required = false) but is not an Option type",
                    arg.name
                );
            }
        }
    }

    // arg attributes must be one of the function arguments
    for arg in &tool_attr.args {
        if !arg_names.iter().any(|ident| **ident == arg.name) {
            panic!("Argument {} not found in function arguments", arg.name);
        }
    }

    // arg attributes must have a description
    for arg in &tool_attr.args {
        if arg.description.is_none() {
            panic!("Argument {} must have a description", arg.name);
        }
    }

    // an arg can not appear more than once, otherwise will panic
    let mut arg_names_set = std::collections::HashSet::new();
    for arg in &tool_attr.args {
        if arg_names_set.contains(&arg.name) {
            panic!("Argument {} appears more than once", arg.name);
        }
        arg_names_set.insert(arg.name.clone());
    }

    let arg_descriptions: Vec<_> = arg_names
        .iter()
        .map(|ident| {
            let arg_meta = tool_attr.args.iter().find(|arg| **ident == arg.name);
            arg_meta
                .and_then(|arg| arg.description.clone())
                .unwrap_or_else(|| format!("Parameter {}", ident.unraw()))
        })
        .collect();

    // Collect required arguments: `Option` args are optional unless marked `required = true`.
    let required_args: Vec<_> = args
        .iter()
        .filter(|(ident, ty)| {
            let arg_meta = tool_attr.args.iter().find(|arg| **ident == arg.name);
            arg_meta
                .and_then(|arg| arg.required)
                .unwrap_or_else(|| !is_option(ty))
        })
        .map(|(ident, _)| ident.unraw().to_string())
        .collect();

    // `{Name}Args` would clash with an argument type of the same name
    // (e.g. `fn search(args: SearchArgs)`), so fall back to `{Name}ToolArgs` then.
    let args_struct_name = {
        let name = quote::format_ident!("{}Args", to_pascal_case(&tool_name));
        // Look through generics too, e.g. `Option<SearchArgs>` or `Vec<SearchArgs>`.
        let clashes = arg_types
            .iter()
            .any(|ty| mentions_ident(ty.to_token_stream(), &name));
        if clashes {
            quote::format_ident!("{}ToolArgs", to_pascal_case(&tool_name))
        } else {
            name
        }
    };

    let call_impl = if input_fn.sig.asyncness.is_some() {
        quote! {
            async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
                #fn_name(#(args.#arg_names),*).await
            }
        }
    } else {
        quote! {
            async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
                #fn_name(#(args.#arg_names),*)
            }
        }
    };
    // Modify the definition implementation to use the description
    let description = match tool_attr.description {
        Some(desc) => quote! { #desc.to_string() },
        None => quote! { format!("Function to {}", Self::NAME) },
    };

    let definition_impl = if !is_struct_args {
        let required_field = if !required_args.is_empty() {
            quote! {
                "required": [#(#required_args),*],
            }
        } else {
            quote! {}
        };

        quote! {
            fn definition(&self) -> swarms_rs::llm::request::ToolDefinition {
                swarms_rs::llm::request::ToolDefinition {
                    name: Self::NAME.to_string(),
                    description: #description,
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            #(
                                #arg_json_names: {
                                    #json_types,
                                    "description": #arg_descriptions
                                }
                            ),*
                        },
                        #required_field
                    }),
                }
            }
        }
    } else {
        quote! {
            fn definition(&self) -> swarms_rs::llm::request::ToolDefinition {
                swarms_rs::llm::request::ToolDefinition {
                    name: Self::NAME.to_string(),
                    description: #description,
                    parameters: serde_json::to_value(schemars::schema_for!(#args_struct_name)).unwrap(),
                }
            }
        }
    };

    let expanded = quote! {
        #[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
        pub struct #struct_name;

        #[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
        pub struct #args_struct_name {
            #(#arg_names: #arg_types),*
        }

        #input_fn

        impl swarms_rs::structs::tool::Tool for #struct_name {
            const NAME: &'static str = #tool_name;

            type Error = #error_type;
            type Args = #args_struct_name;
            type Output = #return_type;

            #definition_impl

            #call_impl
        }

        pub static #static_name: #struct_name = #struct_name;
    };

    expanded.into()
}
