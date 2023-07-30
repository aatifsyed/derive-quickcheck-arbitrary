use derive_quickcheck_arbitrary::Arbitrary;

#[derive(Clone, Arbitrary)]
struct Foo {
    #[arbitrary(skip)]
    #[arbitrary(gen(some_fn))]
    _foo: (),
}

fn main() {}
