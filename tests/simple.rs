use derive_quickcheck_arbitrary::Arbitrary;
use quickcheck::quickcheck;

#[derive(Debug, Clone, Arbitrary)]
struct Yak {
    _name: String,
    _id: usize,
    #[arbitrary(gen(|_| DoesNotImplArbitrary))]
    _does_not_impl_arbitrary: DoesNotImplArbitrary,
    #[arbitrary(gen(|_|String::new()))]
    _empty: String,
    #[arbitrary(default)]
    defaulted: bool,
}

#[derive(Clone, Debug)]
struct DoesNotImplArbitrary;

#[derive(Debug, Clone, Arbitrary)]
enum Shaver {
    Standard,
    Custom {
        _comb_length: usize,
    },
    Named(String),
    #[arbitrary(skip)]
    _Skipped,
    Empty(#[arbitrary(gen(|_|String::new()))] String),
}

#[derive(Debug, Clone, Arbitrary)]
#[arbitrary(where(T: Default + Clone + 'static))]
struct GenericYak<T> {
    #[arbitrary(default)]
    inner: T,
}

quickcheck! {
    fn can_generate_struct(yak: Yak) -> () {
        assert!(!yak.defaulted);
    }

    fn can_generate_generic_struct(yak: GenericYak<String>) -> () {
        assert!(yak.inner.is_empty());
    }

    fn can_generate_enum(shaver: Shaver) -> bool {
        !matches!(shaver, Shaver::_Skipped)
    }
}
