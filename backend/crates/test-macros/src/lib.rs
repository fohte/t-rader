use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Pat, parse_macro_input, parse_quote};

/// DatabaseHandle を受け取る DB テストを、共有 DB の transaction を使う Tokio test にする。
#[proc_macro_attribute]
pub fn database_test(attribute: TokenStream, item: TokenStream) -> TokenStream {
    if !attribute.is_empty() {
        return syn::Error::new(
            proc_macro::Span::call_site().into(),
            "database_test takes no arguments",
        )
        .to_compile_error()
        .into();
    }

    let mut function = parse_macro_input!(item as ItemFn);
    if function.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &function.sig,
            "database_test expects one named database argument",
        )
        .to_compile_error()
        .into();
    }
    let Some(FnArg::Typed(argument)) = function.sig.inputs.first() else {
        return syn::Error::new_spanned(
            &function.sig,
            "database_test expects one named database argument",
        )
        .to_compile_error()
        .into();
    };
    let Pat::Ident(argument_name) = argument.pat.as_ref() else {
        return syn::Error::new_spanned(
            &argument.pat,
            "database_test expects a named database argument",
        )
        .to_compile_error()
        .into();
    };
    let argument_name = argument_name.ident.clone();
    let argument_type = argument.ty.clone();
    let test_name = function.sig.ident.clone();

    if function.sig.asyncness.is_none() {
        return syn::Error::new_spanned(&function.sig, "database_test expects an async function")
            .to_compile_error()
            .into();
    }

    function.sig.inputs.clear();
    function.attrs.push(parse_quote!(#[tokio::test]));
    function.block.stmts.insert(
        0,
        parse_quote!(
            let #argument_name: #argument_type = crate::testing::create_test_transaction(
                concat!(module_path!(), "::", stringify!(#test_name))
            )
            .await;
        ),
    );

    quote!(#function).into()
}
