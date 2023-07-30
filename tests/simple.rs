use derive_quickcheck_arbitrary::Arbitrary;
use quickcheck::quickcheck;

#[derive(Debug, Clone, Arbitrary)]
struct Yak {
    _name: String,
    _id: usize,
    #[arbitrary(gen(|_| DoesNotImplArbitrary))]
    _does_not_impl_arbitrary: DoesNotImplArbitrary,
}

#[derive(Clone, Debug)]
struct DoesNotImplArbitrary;

quickcheck! {
    fn can_generate_struct(_yak: Yak) -> bool {
        true
    }
}
