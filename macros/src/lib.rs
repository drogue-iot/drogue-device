#![allow(incomplete_features)]
#![feature(proc_macro_diagnostic)]

extern crate proc_macro;

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{self, Data, DataStruct, Fields};

fn is_context(field: &syn::Field) -> bool {
    if let syn::Type::Path(p) = &field.ty {
        for s in p.path.segments.iter() {
            if s.ident == "ActorContext" || s.ident == "PackageContext" {
                return true;
            }
        }
    }
    return false;
}

#[proc_macro_derive(Device)]
pub fn device_macro_derive(input: TokenStream) -> TokenStream {
    let input: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };

    let field_name = fields
        .iter()
        .filter(|field| is_context(field))
        .map(|field| &field.ident);
    let field_type = fields
        .iter()
        .filter(|field| is_context(field))
        .map(|field| &field.ty);

    let gen = quote! {
        impl Device for #name {
            fn start(&'static self, spawner: ::drogue_device::reexport::embassy::executor::Spawner) {
                #(
                    #[::drogue_device::reexport::embassy::task(embassy_prefix = "::drogue_device::reexport::")]
                    async fn #field_name(spawner: ::drogue_device::reexport::embassy::executor::Spawner, runner: &'static #field_type) {
                        runner.start(spawner).await
                    }

                    spawner.spawn(#field_name(spawner, &self.#field_name)).unwrap();
                )*
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(Package)]
pub fn package_macro_derive(input: TokenStream) -> TokenStream {
    let input: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };
    let field_name = fields
        .iter()
        .filter(|field| is_context(field))
        .map(|field| &field.ident);
    let field_type = fields
        .iter()
        .filter(|field| is_context(field))
        .map(|field| &field.ty);

    let gen = quote! {
        impl Package for #name {
            fn start(&'static self, spawner: ::drogue_device::reexport::embassy::executor::Spawner) -> ImmediateFuture {
                #(
                    #[::drogue_device::reexport::embassy::task(embassy_prefix = "::drogue_device::reexport::")]
                    async fn #field_name(spawner: ::drogue_device::reexport::embassy::executor::Spawner, runner: &'static #field_type) {
                        runner.start(spawner).await
                    }

                    spawner.spawn(#field_name(spawner, &self.#field_name)).unwrap();
                )*
                ImmediateFuture{}
            }
        }
    };
    gen.into()
}

#[derive(Debug, FromMeta)]
struct MainArgs {
    #[darling(default)]
    config: Option<syn::LitStr>,
}

#[proc_macro_attribute]
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    let macro_args = syn::parse_macro_input!(args as syn::AttributeArgs);
    let task_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let macro_args = match MainArgs::from_list(&macro_args) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    let mut fail = false;
    if task_fn.sig.asyncness.is_none() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("main function must be async")
            .emit();
        fail = true;
    }
    if !task_fn.sig.generics.params.is_empty() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("main function must not be generic")
            .emit();
        fail = true;
    }

    let args = task_fn.sig.inputs.clone();

    if args.len() != 1 {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("main function must have one argument")
            .emit();
        fail = true;
    }

    let mut device_type = None;
    if let syn::FnArg::Typed(t) = &args[0] {
        if let syn::Type::Path(ref tp) = *t.ty {
            if tp.path.segments[0].ident == "DeviceContext" {
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
            .error("main function device context argument must take a generic type parameter implementing Device trait")
            .emit();
        fail = true;
    }

    if fail {
        return TokenStream::new();
    }

    let device_type = device_type.take().unwrap();
    let task_fn_body = task_fn.block;
    let config = macro_args.config;

    let mut result = quote! {

        static DEVICE: ::drogue_device::reexport::embassy::util::Forever<#device_type> = ::drogue_device::reexport::embassy::util::Forever::new();

        #[::drogue_device::reexport::embassy::task(embassy_prefix= "::drogue_device::reexport::")]
        async fn __drogue_main(#args) {
            #task_fn_body
        }
    };

    if let Some(config) = config {
        result = quote! {
            #result

            #[::drogue_device::reexport::embassy::main(embassy_prefix = "::drogue_device::reexport::", config = #config)]
            async fn main(spawner: ::drogue_device::reexport::embassy::executor::Spawner) {
                let context = DeviceContext::new(spawner, &DEVICE);
                spawner.spawn(__drogue_main(context)).unwrap();
            }
        };
    } else {
        result = quote! {
            #result

            #[::drogue_device::reexport::embassy::main(embassy_prefix = "::drogue_device::reexport::")]
            async fn main(spawner: ::drogue_device::reexport::embassy::executor::Spawner) {
                let context = DeviceContext::new(spawner, &DEVICE);
                spawner.spawn(__drogue_main(context)).unwrap();
            }
        };
    }
    result.into()
}

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

    if args.len() != 1 {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("test function must have one argument")
            .emit();
        fail = true;
    }

    let mut device_type = None;
    if let syn::FnArg::Typed(t) = &args[0] {
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

        #[::drogue_device::reexport::embassy::task(embassy_prefix= "::drogue_device::reexport::")]
        async fn #drogue_test_name(#args) {
            #task_fn_body
        }

        #[test]
        fn #test_name() {
            static DEVICE: ::drogue_device::reexport::embassy::util::Forever<#device_type> = ::drogue_device::reexport::embassy::util::Forever::new();
            static RUNNER: ::drogue_device::reexport::embassy::util::Forever<TestRunner> = ::drogue_device::reexport::embassy::util::Forever::new();

            let runner = RUNNER.put(TestRunner::new());

            runner.initialize(|spawner| {
                let context = DeviceContext::new(spawner, &DEVICE);
                let runner = unsafe { RUNNER.steal() };
                spawner.spawn(#drogue_test_name(TestContext::new(runner, context))).unwrap();
            });

            while !runner.is_done() {
                runner.run_until_idle();
            }
        }
    };
    result.into()
}
