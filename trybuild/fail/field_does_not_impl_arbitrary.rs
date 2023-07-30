use derive_quickcheck_arbitrary::Arbitrary;
use quickcheck::Arbitrary;

fn assert_impl_arbitrary<T: Arbitrary>() {}

#[derive(Clone, Arbitrary)]
struct Foo {
    _does_not_impl_arbitrary: DoesNotImplArbitrary,
}

#[derive(Clone)]
struct DoesNotImplArbitrary;

fn main() {
    assert_impl_arbitrary::<Foo>();
}
