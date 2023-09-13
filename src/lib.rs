//! Derive macro for [`quickcheck::Arbitrary`](https://docs.rs/quickcheck/latest/quickcheck/trait.Arbitrary.html).
//!
//! Expands to calling [`Arbitrary::arbitrary`](https://docs.rs/quickcheck/latest/quickcheck/trait.Arbitrary.html#tymethod.arbitrary)
//! on every field of a struct.
//!
//! ```
//! use derive_quickcheck_arbitrary::Arbitrary;
//!
//! #[derive(Clone, Arbitrary)]
//! struct Yakshaver {
//!     id: usize,
//!     name: String,
//! }
//! ```
//!
//! You can customise field generation by either:
//! - providing a callable that accepts [`&mut quickcheck::Gen`](https://docs.rs/quickcheck/latest/quickcheck/struct.Gen.html).
//! - always using the default value
//! ```
//! # use derive_quickcheck_arbitrary::Arbitrary;
//! # mod num { pub fn clamp(input: usize, min: usize, max: usize) -> usize { todo!() } }
//! #[derive(Clone, Arbitrary)]
//! struct Yakshaver {
//!     /// Must be less than 10_000
//!     #[arbitrary(gen(|g| num::clamp(usize::arbitrary(g), 0, 10_000) ))]
//!     id: usize,
//!     name: String,
//!     #[arbitrary(default)]
//!     always_false: bool,
//! }
//! ```
//!
//! You can skip enum variants:
//! ```
//! # use derive_quickcheck_arbitrary::Arbitrary;
//! #[derive(Clone, Arbitrary)]
//! enum YakType {
//!     Domestic {
//!         name: String,
//!     },
//!     Wild,
//!     #[arbitrary(skip)]
//!     Alien,
//! }
//! ```
//!
//! You can add bounds for generic structs:
//! ```
//! # use derive_quickcheck_arbitrary::Arbitrary;
//! # use quickcheck::Arbitrary;
//! #[derive(Clone, Arbitrary)]
//! #[arbitrary(where(T: Arbitrary))]
//! struct GenericYak<T> {
//!     name: T,
//! }
//! ```

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens as _};
use structmeta::{NameArgs, StructMeta};
use syn::{
    parse::{Parse, ParseStream, Parser as _},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned as _,
    token::{Brace, Colon, Comma},
    AttrStyle, Attribute, DataEnum, DataStruct, DeriveInput, Expr, ExprStruct, FieldValue, Fields,
    Index, Member, Path, PathSegment, Token, Variant, WhereClause, WherePredicate,
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
    let generics = input.generics.clone();
    let gen_name = &quote!(g);
    let predicates = match get_one_arg(&input.attrs, input.span())? {
        Some(Arg::Where(preds)) => preds,
        None => Punctuated::new(),
        Some(Arg::Default | Arg::Gen(_) | Arg::Skip) => {
            return Err(syn::Error::new(
                input.span(),
                "only `where` is valid for items",
            ))
        }
    };
    let where_clause = WhereClause {
        where_token: Token![where](Span::call_site()),
        predicates,
    };

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
                     }| match get_one_arg(&attrs, span) {
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
                        Ok(Some(Arg::Gen(_) | Arg::Default | Arg::Where(_))) => {
                            Some(Err(syn::Error::new(
                                span,
                                "`gen`, `default` and `where` are not valid for enum variants", // TODO: probably could be
                            )))
                        }
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
        impl #generics ::quickcheck::Arbitrary for #struct_name #generics
            #where_clause
        {
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
            let value = match get_one_arg(&field.attrs, field.span())? {
                Some(Arg::Skip | Arg::Where(_)) => {
                    return Err(syn::Error::new_spanned(
                        field,
                        "`skip` and `where` are not valid for members",
                    ))
                }
                Some(Arg::Gen(custom)) => {
                    let ty = field.ty;
                    quote! {
                        (
                            ( #custom ) as ( fn(&mut ::quickcheck::Gen) -> #ty )
                        ) // cast to fn pointer
                        (&mut *#gen_name) // call it
                    }
                }
                Some(Arg::Default) => {
                    quote!(::core::default::Default::default())
                }
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
    Default,
    Where(Punctuated<WherePredicate, Comma>),
}

#[derive(StructMeta, Debug, Default)]
struct AttrArgs {
    gen: Option<NameArgs<TokenStream>>,
    skip: bool,
    default: bool,
    r#where: Option<NameArgs<TokenStream>>,
}

impl Parse for Arg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut hint = syn::Error::new(
            input.span(),
            "expected one of  `gen`, `default`, `where` or `skip`",
        );
        match AttrArgs::parse(input) {
            // inner error
            Err(e) => {
                hint.combine(e);
                Err(hint)
            }
            // nothing
            Ok(AttrArgs {
                gen: None,
                r#where: None,
                skip: false,
                default: false,
            }) => Err(hint),
            // just `skip`
            Ok(AttrArgs {
                skip: true,

                gen: None,
                default: false,
                r#where: None,
            }) => Ok(Arg::Skip),
            // just `gen`
            Ok(AttrArgs {
                gen: Some(NameArgs { name_span: _, args }),

                r#where: None,
                skip: false,
                default: false,
            }) => Ok(Arg::Gen(args)),

            // just `where`
            Ok(AttrArgs {
                r#where: Some(NameArgs { name_span: _, args }),

                gen: None,
                skip: false,
                default: false,
            }) => Ok(Arg::Where(Punctuated::parse_terminated.parse2(args)?)), // just `default`
            Ok(AttrArgs {
                default: true,

                r#where: None,
                gen: None,
                skip: false,
            }) => Ok(Arg::Default),
            // some combination of arguments
            Ok(AttrArgs { .. }) => Err(hint),
        }
    }
}

fn get_one_arg(attrs: &[Attribute], parent_span: Span) -> syn::Result<Option<Arg>> {
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
    fn readme() {
        assert!(
            std::process::Command::new("cargo")
                .args(["rdme", "--check"])
                .output()
                .expect("couldn't run `cargo rdme`")
                .status
                .success(),
            "README.md is out of date - bless the new version by running `cargo rdme`"
        )
    }

    #[test]
    fn attr_args() {
        assert_eq!(
            AttrArgs {
                skip: true,
                ..Default::default()
            },
            parse_quote!(skip),
        );
        assert_eq!(
            AttrArgs {
                default: true,
                ..Default::default()
            },
            parse_quote!(default),
        );
        assert_eq!(
            AttrArgs {
                gen: Some(NameArgs {
                    name_span: Span::call_site(),
                    args: quote!(some_fn)
                }),
                ..Default::default()
            },
            parse_quote!(gen(some_fn)),
        );
        assert_eq!(
            AttrArgs {
                r#where: Some(NameArgs {
                    name_span: Span::call_site(),
                    args: quote!(foo)
                }),
                ..Default::default()
            },
            parse_quote!(where(foo)),
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
            fn norm(t: &AttrArgs) -> (Option<String>, &bool, &bool, Option<String>) {
                let AttrArgs {
                    gen,
                    skip,
                    default,
                    r#where,
                } = t;
                (
                    gen.as_ref().map(|it| it.args.to_string()),
                    skip,
                    default,
                    r#where.as_ref().map(|it| it.args.to_string()),
                )
            }
            norm(self) == norm(other)
        }
    }
}
