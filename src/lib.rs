use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens as _};
use structmeta::{NameArgs, StructMeta};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned as _,
    token::{Brace, Colon, Comma},
    AttrStyle, Attribute, DataEnum, DataStruct, DeriveInput, Expr, ExprStruct, FieldValue, Fields,
    Index, Member, Path, PathSegment, Variant,
};

// TODO: https://docs.rs/proc-macro-crate/latest/proc_macro_crate/
// TODO: https://crates.io/crates/parse-variants

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
        syn::Data::Struct(DataStruct { fields, .. }) => expr_struct(
            path_of_idents([struct_name.clone()]),
            field_values(fields, gen_name)?,
        )
        .into_token_stream(),
        syn::Data::Enum(DataEnum { variants, .. }) => {
            let span = variants.span();
            let variant_ctors = variants
                .into_iter()
                .filter_map(
                    |Variant {
                         attrs,
                         ident,
                         fields,
                         ..
                     }| match get_arg(&attrs, span) {
                        Ok(None) => match field_values(fields, gen_name) {
                            Ok(fields) => {
                                let variant_ctor = expr_struct(
                                    path_of_idents([struct_name.clone(), ident]),
                                    fields,
                                );
                                Some(Ok(variant_ctor))
                            }
                            Err(e) => Some(Err(e)),
                        },
                        Ok(Some(Arg::Skip)) => None,
                        Ok(Some(Arg::Gen(arg))) => Some(Err(syn::Error::new_spanned(
                            arg,
                            "`gen` is not valid for enum variants", // TODO: probably could be
                        ))),
                        Err(e) => Some(Err(e)),
                    },
                )
                .collect::<Result<Vec<_>, _>>()?;
            quote!(
                let options = [ #(#variant_ctors,)* ];
                #gen_name.choose(options.as_slice()).expect("no variants to choose from").clone()
            )
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

fn field_values(
    fields: Fields,
    gen_name: &TokenStream,
) -> syn::Result<Punctuated<FieldValue, Comma>> {
    fields
        .into_iter()
        .enumerate()
        .map(|(ix, field)| {
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
        })
        .collect()
}

fn expr_struct(path: Path, field_values: Punctuated<FieldValue, Comma>) -> ExprStruct {
    ExprStruct {
        attrs: vec![],
        qself: None,
        path,
        brace_token: Brace::default(),
        fields: field_values,
        dot2_token: None,
        rest: None,
    }
}

fn path_of_idents(idents: impl IntoIterator<Item = Ident>) -> Path {
    Path {
        leading_colon: None,
        segments: Punctuated::from_iter(idents.into_iter().map(|ident| PathSegment {
            ident,
            arguments: syn::PathArguments::None,
        })),
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
