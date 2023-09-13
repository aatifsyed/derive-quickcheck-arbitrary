<!-- cargo-rdme start -->

Derive macro for [`quickcheck::Arbitrary`](https://docs.rs/quickcheck/latest/quickcheck/trait.Arbitrary.html).

Expands to calling [`Arbitrary::arbitrary`](https://docs.rs/quickcheck/latest/quickcheck/trait.Arbitrary.html#tymethod.arbitrary)
on every field of a struct.

```rust
use derive_quickcheck_arbitrary::Arbitrary;

#[derive(Clone, Arbitrary)]
struct Yakshaver {
    id: usize,
    name: String,
}
```

You can customise field generation by either:
- providing a callable that accepts [`&mut quickcheck::Gen`](https://docs.rs/quickcheck/latest/quickcheck/struct.Gen.html).
- always using the default value
```rust
#[derive(Clone, Arbitrary)]
struct Yakshaver {
    /// Must be less than 10_000
    #[arbitrary(gen(|g| num::clamp(usize::arbitrary(g), 0, 10_000) ))]
    id: usize,
    name: String,
    #[arbitrary(default)]
    always_false: bool,
}
```

You can skip enum variants:
```rust
#[derive(Clone, Arbitrary)]
enum YakType {
    Domestic {
        name: String,
    },
    Wild,
    #[arbitrary(skip)]
    Alien,
}
```

You can add bounds for generic structs:
```rust
#[derive(Clone, Arbitrary)]
#[arbitrary(where(T: Arbitrary))]
struct GenericYak<T> {
    name: T,
}
```

<!-- cargo-rdme end -->
