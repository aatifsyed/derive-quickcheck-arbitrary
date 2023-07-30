use derive_quickcheck_arbitrary::Arbitrary;

#[derive(Clone, Arbitrary)]
struct Foo {
    #[arbitrary(does_not_exist)]
    _foo: (),
}

#[derive(Clone, Arbitrary)]
struct Bar {
    #[arbitrary(skip)]
    _bar: (),
}

fn main() {}
