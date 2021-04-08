#![allow(incomplete_features)]
#![feature(proc_macro_diagnostic)]
#![feature(concat_idents)]

extern crate proc_macro;

// use darling::FromMeta;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{self, Data, DataStruct, Fields};

/*
#[proc_macro_derive(ActorProcess)]
pub fn actor_process_macro_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    let name = format_ident!("{}", name);
    let handler = format!("__drogue_{}_trampoline", name);
    //    let lowercase_name = Ident::new(&name.to_string().to_lowercase(), name.span());
    let gen = quote! {

        extern crate embassy;
        use embassy;

        #[embassy::task]
        async fn #handler(state: &'static ActorState<'static, #name>) {
            let channel = &state.channel;
            let mut actor = state.actor.borrow_mut();
            loop {
                let request = channel.receive().await;
                #name::process(&mut actor, request).await;
            }
        }
    };
    gen.into()
}*/

/*
#[proc_macro]
pub fn bind(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    dbg!(&ast);
    let gen = quote! {
        #ast
    };
    gen.into()
    //let name = syn::parse_macro_input!(item as syn::Ident);
    //let name = format!("{}", name);
    //let name_interrupt = format_ident!("{}", name);
    //let name_handler = format!("__EMBASSY_{}_HANDLER", name);
}
*/

#[proc_macro_attribute]
pub fn actor(_: TokenStream, item: TokenStream) -> TokenStream {
    // let macro_args = syn::parse_macro_input!(args as syn::AttributeArgs);
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
            .error("actor function must not be generic")
            .emit();
        fail = true;
    }

    let args = task_fn.sig.inputs.clone();

    if args.len() != 2 {
        task_fn
            .sig
            .span()
            .unwrap()
            .error("actor function must take two arguments")
            .emit();
        fail = true;
    }

    let actor_type = match &args[0] {
        syn::FnArg::Typed(t) => match &*t.ty {
            syn::Type::Reference(tref) => match &*tref.elem {
                syn::Type::Path(p) => p.path.get_ident().clone(),
                _ => {
                    task_fn
                        .sig
                        .span()
                        .unwrap()
                        .error("actor type must be a specific type")
                        .emit();
                    fail = true;
                    None
                }
            },
            _ => {
                task_fn
                    .sig
                    .span()
                    .unwrap()
                    .error("actor argument must be a type reference")
                    .emit();
                fail = true;
                None
            }
        },
        _ => {
            task_fn
                .sig
                .span()
                .unwrap()
                .error("first argument must be an actor")
                .emit();
            fail = true;
            None
        }
    };

    let message_type = {
        match &args[1] {
            syn::FnArg::Typed(t) => match &*t.ty {
                syn::Type::Path(p) => p.path.get_ident().clone(),
                _ => {
                    task_fn
                        .sig
                        .span()
                        .unwrap()
                        .error("message type argument must refer to a specific type")
                        .emit();
                    fail = true;
                    None
                }
            },
            _ => {
                task_fn
                    .sig
                    .span()
                    .unwrap()
                    .error("second argument must be a message type")
                    .emit();
                fail = true;
                None
            }
        }
    };

    if fail {
        return TokenStream::new();
    }

    let actor_type = actor_type.unwrap();
    let message_type = message_type.unwrap();
    let name = task_fn.sig.ident.clone();
    let result = quote! {
        #task_fn


        impl Actor for #actor_type {
            type Message = #message_type;
        }

    };
    result.into()
}

#[proc_macro_derive(Device)]
pub fn device_macro_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
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

    let field_name2 = fields.iter().map(|field| &field.ident);

    dbg!(&fields);
    let gen = quote! {
        #(
            mod #field_name {
                use drogue_device_platform_std::{Actor, ActorState, Forever};
                pub static DEVICE_ACTOR: Forever<#field_type> = Forever::new();

                #[embassy::task]
                pub async fn trampoline(state: &'static #field_type) {
                    let channel = &state.channel;
                    let mut actor = unsafe { (&mut *state.actor.get()) };
                    loop {
                        let mut pinned = core::pin::Pin::new(&mut *actor);
                        let request = channel.receive().await;
                        pinned.process(request).await;
                    }
                }
            }
        )*

        impl Device for #name {
            fn mount(&'static self, spawner: embassy::executor::Spawner) {
                #(
                    self.#field_name2.mount();
                    spawner.spawn(#field_name2::trampoline(&self.#field_name2));
                )*
            }
        }
    };
    gen.into()
}

#[proc_macro_attribute]
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    //let macro_args = syn::parse_macro_input!(args as syn::AttributeArgs);
    let cfg: syn::Ident = syn::parse(args).unwrap();
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

    let task_fn_body = task_fn.block.clone();

    let result = quote! {

        #[embassy::task]
        async fn __drogue_main(#args) {
            #task_fn_body
        }

        // TODO: Cortex-mi'ify #[cortex_m_rt::entry]
        fn main() -> ! {
            unsafe fn make_static<T>(t: &mut T) -> &'static mut T {
                ::core::mem::transmute(t)
            }

            let mut executor = embassy_std::Executor::new();
            let executor = unsafe { make_static(&mut executor) };
            let mut device = #cfg();
            let device = unsafe { make_static(&mut device) };
            let context = DeviceContext::new(device);

            executor.run(|spawner| {
                context.device().mount(spawner);
                spawner.spawn(__drogue_main(context)).unwrap();
            })

        }
    };
    result.into()
}
