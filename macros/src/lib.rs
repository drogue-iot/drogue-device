#![allow(incomplete_features)]
#![feature(proc_macro_diagnostic)]

extern crate proc_macro;
mod configure;

use configure::configure;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{self};

#[proc_macro_attribute]
pub fn test(_: TokenStream, item: TokenStream) -> TokenStream {
    let task_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let mut fail = false;
    if task_fn.sig.asyncness.is_none() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("test function must be async")
            .emit();
        fail = true;
    }
    if !task_fn.sig.generics.params.is_empty() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("test function must not be generic")
            .emit();
        fail = true;
    }

    let args = task_fn.sig.inputs.clone();

    if args.len() != 2 {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("test function must take two arguments")
            .emit();
        fail = true;
    }

    let mut device_type = None;
    if let syn::FnArg::Typed(t) = &args[1] {
        if let syn::Type::Path(ref tp) = *t.ty {
            if tp.path.segments[0].ident == "TestContext" {
                if let syn::PathArguments::AngleBracketed(args) = &tp.path.segments[0].arguments {
                    for arg in args.args.iter() {
                        if let syn::GenericArgument::Type(syn::Type::Path(tp)) = arg {
                            device_type = tp.path.get_ident();
                            break;
                        }
                    }
                }
            }
        }
    };

    if device_type.is_none() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("main function test context argument must take a generic type parameter implementing Device trait")
            .emit();
        fail = true;
    }

    if fail {
        return TokenStream::new();
    }

    let test_name = task_fn.sig.ident;
    let device_type = device_type.take().unwrap();
    let task_fn_body = task_fn.block;
    let drogue_test_name = format_ident!("__drogue_test_{}", test_name);

    let result = quote! {

        #[::embassy_executor::task]
        async fn #drogue_test_name(#args) {
            #task_fn_body
        }

        #[test]
        fn #test_name() {
            static DEVICE: ::embassy_util::Forever<#device_type> = ::embassy_util::Forever::new();
            static RUNNER: ::embassy_util::Forever<::ector::testutil::TestRunner> = ::embassy_util::Forever::new();

            let runner = RUNNER.put(::ector::testutil::TestRunner::default());

            runner.initialize(|spawner| {
                let runner = unsafe { RUNNER.steal() };
                spawner.spawn(#drogue_test_name(spawner, ::ector::testutil::TestContext::new(runner, &DEVICE))).unwrap();
            });

            while !runner.is_done() {
                runner.run_until_idle();
            }
        }
    };
    result.into()
}

#[proc_macro]
pub fn log_stack(_item: TokenStream) -> TokenStream {
    let result = quote! {
        crate::print_stack(file!(), line!());
    };
    result.into()
}

#[proc_macro]
pub fn config(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::LitStr);
    let s = input.value();
    let output = configure(&s);
    quote!(#output).into()
}
