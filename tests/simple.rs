use derive_quickcheck_arbitrary::Arbitrary;
use quickcheck::quickcheck;

#[derive(Debug, Clone, Arbitrary)]
struct Yak {
    _name: String,
    _id: usize,
}

quickcheck! {
    fn can_generate_struct(_yak: Yak) -> bool {
        true
    }
}
