#![allow(incomplete_features)]
#![feature(proc_macro_diagnostic)]

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{self, Data, DataStruct, Fields};

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
    let field_name = fields.iter().map(|field| &field.ident);
    let field_type = fields.iter().map(|field| &field.ty);

    let gen = quote! {
        impl Device for #name {
            fn mount(&'static self, spawner: embassy::executor::Spawner) {
                #(
                    #[embassy::task]
                    async fn #field_name(state: &'static #field_type) {
                        let channel = &state.channel;
                        let mut actor = unsafe { (&mut *state.actor.get()) };

                        core::pin::Pin::new(&mut *actor).on_start().await;
                        loop {
                            let mut pinned = core::pin::Pin::new(&mut *actor);
                            let mut message = channel.receive().await;
                            pinned.on_message(message.take_message()).await;
                        }
                    }

                    self.#field_name.mount();
                    spawner.spawn(#field_name(&self.#field_name)).unwrap();
                )*
            }
        }
    };
    gen.into()
}

#[proc_macro_attribute]
pub fn configure(_: TokenStream, item: TokenStream) -> TokenStream {
    let task_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let mut fail = false;
    if !task_fn.sig.asyncness.is_none() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("configure function must be sync")
            .emit();
        fail = true;
    }
    if !task_fn.sig.generics.params.is_empty() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("configure function must not be generic")
            .emit();
        fail = true;
    }

    let args = task_fn.sig.inputs.clone();
    if args.len() != 0 {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("configure function must not take any arguments")
            .emit();
        fail = true;
    }

    let device_type: Option<&syn::Ident> = match &task_fn.sig.output {
        syn::ReturnType::Default => {
            task_fn
                .sig
                .span()
                .unwrap()
                .error("return type must be specified")
                .emit();
            fail = true;
            None
        }
        syn::ReturnType::Type(_, v) => match &**v {
            syn::Type::Path(t) => t.path.get_ident(),
            _ => {
                task_fn
                    .sig
                    .span()
                    .unwrap()
                    .error("return type must be a path type")
                    .emit();
                fail = true;
                None
            }
        },
    };

    if fail {
        return TokenStream::new();
    }

    let device_type = device_type.unwrap();
    let task_fn_body = task_fn.block;

    let result = quote! {

        static DEVICE: embassy::util::Forever<#device_type> = embassy::util::Forever::new();

        fn __drogue_configure() -> &'static #device_type {
            let device = #task_fn_body;
            DEVICE.put(device)
        }
    };
    result.into()
}

#[proc_macro_attribute]
pub fn main(_: TokenStream, item: TokenStream) -> TokenStream {
    let task_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let mut fail = false;
    if task_fn.sig.asyncness.is_none() {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("task functions must be async")
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

    if fail {
        return TokenStream::new();
    }

    let task_fn_body = task_fn.block;

    let result = quote! {

        static EXECUTOR: embassy::util::Forever<embassy_std::Executor> = embassy::util::Forever::new();

        #[embassy::task]
        async fn __drogue_main(#args) {
            #task_fn_body
        }

        // TODO: Cortex-mi'ify #[cortex_m_rt::entry]
        fn main() -> ! {
            let (executor, device) = {
                let executor = EXECUTOR.put(embassy_std::Executor::new());
                let device = __drogue_configure();
                (executor, device)
            };

            let context = DeviceContext::new(device);
            executor.run(|spawner| {
                context.device().mount(spawner);
                spawner.spawn(__drogue_main(context)).unwrap();
            })

        }
    };
    result.into()
}
