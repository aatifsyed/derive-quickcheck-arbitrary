use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

// TODO: https://docs.rs/proc-macro-crate/latest/proc_macro_crate/

#[proc_macro_derive(Arbitrary)]
pub fn derive_arbitrary(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let user_struct = parse_macro_input!(input as DeriveInput);
    expand_arbitrary(&user_struct)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_arbitrary(_input: &DeriveInput) -> syn::Result<TokenStream> {
    Ok(quote! {
        impl ::quickcheck::Arbitrary for Foo {
            fn arbitrary(g: &mut ::quickcheck::Gen) -> Self {
                todo!()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn trybuild() {
        let t = trybuild::TestCases::new();
        t.pass("trybuild/pass/**/*.rs");
        t.compile_fail("trybuild/fail/**/*.rs")
    }
}
