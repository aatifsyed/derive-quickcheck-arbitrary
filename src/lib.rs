use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    token::{Brace, Colon},
    DataStruct, DeriveInput, Expr, ExprStruct, FieldValue, Index, Member, Path, PathSegment,
};

// TODO: https://docs.rs/proc-macro-crate/latest/proc_macro_crate/

#[proc_macro_derive(Arbitrary)]
pub fn derive_arbitrary(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let user_struct = parse_macro_input!(input as DeriveInput);
    expand_arbitrary(user_struct)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_arbitrary(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = input.ident.clone();
    let gen_name = &quote!(g);
    let ctor = match input.data {
        syn::Data::Struct(DataStruct { fields, .. }) => ExprStruct {
            attrs: vec![],
            qself: None,
            path: path_of_ident(struct_name.clone()),
            brace_token: Brace::default(),
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(ix, field)| FieldValue {
                    attrs: vec![],
                    member: match field.ident {
                        Some(name) => Member::Named(name),
                        None => Member::Unnamed(Index::from(ix)),
                    },
                    colon_token: Some(Colon::default()),
                    expr: Expr::Verbatim(quote!(::quickcheck::Arbitrary::arbitrary(#gen_name))),
                })
                .collect(),
            dot2_token: None,
            rest: None,
        },
        syn::Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(Arbitrary)] is not supported on `enum`s",
            ))
        }
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(Arbitrary)] is not supported on `union`s",
            ))
        }
    };

    Ok(quote! {
        impl ::quickcheck::Arbitrary for #struct_name {
            fn arbitrary(#gen_name: &mut ::quickcheck::Gen) -> Self {
                #ctor
            }
        }
    })
}

fn path_of_ident(ident: Ident) -> Path {
    Path {
        leading_colon: None,
        segments: Punctuated::from_iter([PathSegment {
            ident,
            arguments: syn::PathArguments::None,
        }]),
    }
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
