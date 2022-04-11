use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Error, Parse, ParseStream, Result};
use syn::visit_mut::{self, VisitMut};
use syn::{
    parse_quote, parse_quote_spanned, punctuated::Punctuated, spanned::Spanned, Attribute, Block,
    FnArg, GenericArgument, GenericParam, ImplItem, ImplItemType, ItemImpl, Lifetime, Pat,
    Receiver, Signature, Token, Type, TypeReference, WhereClause,
};

pub struct Item(ItemImpl);

impl ToTokens for Item {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens)
    }
}

impl Parse for Item {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let mut lookahead = input.lookahead1();
        if lookahead.peek(Token![unsafe]) {
            let ahead = input.fork();
            ahead.parse::<Token![unsafe]>()?;
            lookahead = ahead.lookahead1();
        }

        if lookahead.peek(Token![impl]) {
            let mut item: ItemImpl = input.parse()?;
            if item.trait_.is_none() {
                return Err(Error::new(Span::call_site(), "expected a trait impl"));
            }
            item.attrs = attrs;
            Ok(Item(item))
        } else {
            Err(lookahead.error())
        }
    }
}

pub struct CollectLifetimes {
    pub elided: Vec<Lifetime>,
    pub explicit: Vec<Lifetime>,
    pub name: &'static str,
    pub default_span: Span,
}

impl CollectLifetimes {
    pub fn new(name: &'static str, default_span: Span) -> Self {
        CollectLifetimes {
            elided: Vec::new(),
            explicit: Vec::new(),
            name,
            default_span,
        }
    }

    fn visit_opt_lifetime(&mut self, lifetime: &mut Option<Lifetime>) {
        match lifetime {
            None => *lifetime = Some(self.next_lifetime(None)),
            Some(lifetime) => self.visit_lifetime(lifetime),
        }
    }

    fn visit_lifetime(&mut self, lifetime: &mut Lifetime) {
        if lifetime.ident == "_" {
            *lifetime = self.next_lifetime(lifetime.span());
        } else {
            self.explicit.push(lifetime.clone());
        }
    }

    fn next_lifetime<S: Into<Option<Span>>>(&mut self, span: S) -> Lifetime {
        let name = format!("{}{}", self.name, self.elided.len());
        let span = span.into().unwrap_or(self.default_span);
        let life = Lifetime::new(&name, span);
        self.elided.push(life.clone());
        life
    }
}

impl VisitMut for CollectLifetimes {
    fn visit_receiver_mut(&mut self, arg: &mut Receiver) {
        if let Some((_, lifetime)) = &mut arg.reference {
            self.visit_opt_lifetime(lifetime);
        }
    }

    fn visit_type_reference_mut(&mut self, ty: &mut TypeReference) {
        self.visit_opt_lifetime(&mut ty.lifetime);
        visit_mut::visit_type_reference_mut(self, ty);
    }

    fn visit_generic_argument_mut(&mut self, gen: &mut GenericArgument) {
        if let GenericArgument::Lifetime(lifetime) = gen {
            self.visit_lifetime(lifetime);
        }
        visit_mut::visit_generic_argument_mut(self, gen);
    }
}

pub(crate) fn generate_actor(input: &mut Item) {
    let Item(input) = input;
    let on_mount_future: ImplItemType = parse_quote! {
        type OnMountFuture<'m, M> = impl core::future::Future<Output = ()> + 'm
            where Self: 'm, M: 'm + Inbox<Self::Message<'m>>;
    };

    input.items.push(ImplItem::Type(on_mount_future));

    let mut lifetimes = CollectLifetimes::new("'impl", input.impl_token.span);
    lifetimes.visit_type_mut(&mut *input.self_ty);
    lifetimes.visit_path_mut(&mut input.trait_.as_mut().unwrap().1);
    let params = &input.generics.params;
    let elided = lifetimes.elided;
    input.generics.params = parse_quote!(#(#elided,)* #params);

    for inner in &mut input.items {
        if let ImplItem::Method(method) = inner {
            let sig = &mut method.sig;
            if sig.asyncness.is_some() {
                let block = &mut method.block;
                transform_sig(sig);
                transform_block(block);
            }
        }
    }
}

fn transform_block(block: &mut Block) {
    let stmts = &block.stmts;
    let let_ret = quote!(#(#stmts)*);
    let async_move = quote_spanned!(block.brace_token.span=>
        async move { #let_ret }
    );
    block.stmts = parse_quote!(#async_move);
}

fn transform_sig(sig: &mut Signature) {
    sig.fn_token.span = sig.asyncness.take().unwrap().span;

    let default_span = sig
        .ident
        .span()
        .join(sig.paren_token.span)
        .unwrap_or_else(|| sig.ident.span());

    for param in sig.generics.params.iter() {
        match param {
            GenericParam::Type(param) => {
                let param = &param.ident;
                let span = param.span();
                where_clause_or_default(&mut sig.generics.where_clause)
                    .predicates
                    .push(parse_quote_spanned!(span=> #param: 'm));
            }
            GenericParam::Lifetime(param) => {
                let param = &param.lifetime;
                let span = param.span();
                where_clause_or_default(&mut sig.generics.where_clause)
                    .predicates
                    .push(parse_quote_spanned!(span=> #param: 'm));
            }
            GenericParam::Const(_) => {}
        }
    }

    if sig.generics.lt_token.is_none() {
        sig.generics.lt_token = Some(Token![<](sig.ident.span()));
    }
    if sig.generics.gt_token.is_none() {
        sig.generics.gt_token = Some(Token![>](sig.paren_token.span));
    }

    sig.generics
        .params
        .push(parse_quote_spanned!(default_span=> 'm));

    let bound_span = sig.ident.span();
    let where_clause = where_clause_or_default(&mut sig.generics.where_clause);
    where_clause
        .predicates
        .push(parse_quote_spanned!(bound_span=> Self: 'm));

    for (_, arg) in sig.inputs.iter_mut().enumerate() {
        match arg {
            FnArg::Receiver(arg) => {
                let s = arg.span().clone();
                let Receiver { reference, .. } = arg;
                if let Some((_, r)) = reference.as_mut() {
                    r.replace(Lifetime::new("'m", s));
                } else {
                    arg.mutability = None;
                }
            }

            FnArg::Typed(arg) => {
                let s = arg.span().clone();
                if let Pat::Ident(ident) = &mut *arg.pat {
                    ident.by_ref = None;
                }

                if let Type::Reference(r) = &mut *arg.ty {
                    r.lifetime.replace(Lifetime::new("'m", s));
                }
            }
        }
    }

    let ret_span = sig.ident.span();
    sig.output = parse_quote_spanned! {ret_span=>
        -> Self::OnMountFuture<'m, M>
    };
}

fn where_clause_or_default(clause: &mut Option<WhereClause>) -> &mut WhereClause {
    clause.get_or_insert_with(|| WhereClause {
        where_token: Default::default(),
        predicates: Punctuated::new(),
    })
}
