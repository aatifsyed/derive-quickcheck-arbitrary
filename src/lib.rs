use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use structmeta::{NameArgs, StructMeta};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned as _,
    token::{Brace, Colon},
    AttrStyle, Attribute, DataStruct, DeriveInput, Expr, ExprStruct, Field, FieldValue, Index,
    Member, Path, PathSegment,
};

// TODO: https://docs.rs/proc-macro-crate/latest/proc_macro_crate/

#[proc_macro_derive(Arbitrary, attributes(arbitrary))]
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
                .map(|(ix, field)| field_value(field, gen_name, ix))
                .collect::<Result<_, _>>()?,
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

fn field_value(field: Field, gen_name: &TokenStream, ix: usize) -> syn::Result<FieldValue> {
    let value = match get_arg(&field.attrs, field.span())? {
        Some(Arg::Skip) => {
            return Err(syn::Error::new_spanned(
                field,
                "`skip` is not valid for members",
            ))
        }
        Some(Arg::Gen(custom)) => quote!((#custom)(#gen_name)),
        None => quote!(::quickcheck::Arbitrary::arbitrary(#gen_name)),
    };
    Ok(FieldValue {
        attrs: vec![],
        member: match field.ident {
            Some(name) => Member::Named(name),
            None => Member::Unnamed(Index::from(ix)),
        },
        colon_token: Some(Colon::default()),
        expr: Expr::Verbatim(value),
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

#[derive(Clone)]
enum Arg {
    Skip,
    Gen(TokenStream),
}

#[derive(StructMeta, Debug)]
struct AttrArgs {
    gen: Option<NameArgs<TokenStream>>,
    skip: bool,
}

impl Parse for Arg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut hint = syn::Error::new(input.span(), "expected arguments `gen` or `skip`");
        match AttrArgs::parse(input) {
            Err(e) => {
                hint.combine(e);
                Err(hint)
            }
            Ok(AttrArgs {
                gen: None,
                skip: false,
            }) => Err(hint),
            Ok(AttrArgs {
                gen: None,
                skip: true,
            }) => Ok(Arg::Skip),
            Ok(AttrArgs {
                gen: Some(NameArgs { name_span: _, args }),
                skip: false,
            }) => Ok(Arg::Gen(args)),
            Ok(AttrArgs {
                gen: Some(_),
                skip: true,
            }) => Err(hint),
        }
    }
}

fn get_arg(attrs: &[Attribute], parent_span: Span) -> syn::Result<Option<Arg>> {
    let configs = attrs
        .iter()
        .filter(|it| it.path().is_ident("arbitrary"))
        .map(
            |attr @ Attribute {
                 pound_token: _,
                 style,
                 bracket_token: _,
                 meta: _,
             }| {
                match style {
                    AttrStyle::Outer => attr.parse_args::<Arg>(),
                    AttrStyle::Inner(_) => Err(syn::Error::new_spanned(
                        attr,
                        "only outer attributes are supported: `#[arbitrary(...)]`",
                    )),
                }
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    match configs.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one.clone())),
        _too_many => Err(syn::Error::new(
            parent_span,
            "`#[arbitrary(...)]` can only be specified once",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use structmeta::NameArgs;
    use syn::parse_quote;

    #[test]
    fn attr_args() {
        assert_eq!(
            AttrArgs {
                skip: true,
                gen: None
            },
            parse_quote!(skip),
        );
        assert_eq!(
            AttrArgs {
                skip: false,
                gen: Some(NameArgs {
                    name_span: Span::call_site(),
                    args: quote!(some_fn)
                })
            },
            parse_quote!(gen(some_fn)),
        );
    }

    #[test]
    fn trybuild() {
        let t = trybuild::TestCases::new();
        t.pass("trybuild/pass/**/*.rs");
        t.compile_fail("trybuild/fail/**/*.rs")
    }

    impl PartialEq for AttrArgs {
        fn eq(&self, other: &Self) -> bool {
            let Self { skip, gen: custom } = self;
            match (custom, &other.gen) {
                (Some(left), Some(right)) => {
                    *skip == other.skip && left.args.to_string() == right.args.to_string()
                }
                (None, None) => *skip == other.skip,
                _ => false,
            }
        }
    }
}
