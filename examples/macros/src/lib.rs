extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    //let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    //let f = syn::parse_macro_input!(item as syn::ItemFn);
    //main::run(args, f).unwrap_or_else(|x| x).into()
    item
}
