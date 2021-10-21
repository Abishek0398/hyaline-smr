# `Hyaline-SMR`

This crate provides garbage collection using [hyaline](https://arxiv.org/pdf/1905.07903.pdf) algorithm for building concurrent data structures.

When a thread removes an object from a concurrent data structure, other threads
may be still using pointers to it at the same time, so it cannot be destroyed
immediately. Hyaline based garbage collection is an alternative to epoch based garabge collection 
to defer the destruction of these shared objects until no pointers to them can exist.

see [Snapshot-Free, Transparent, and Robust Memory
Reclamation for Lock-Free Data Structures](https://arxiv.org/pdf/1905.07903.pdf) for further details.

This crate requires nightly.

[Documentation](https://docs.rs/hyaline_smr)

## Usage
Add this to your `Cargo.toml`:
```toml
[dependencies]
hyaline_smr = "0.1"
```

## Example
Refer [documentation](https://docs.rs/hyaline_smr)

## Credits
[Snapshot-Free, Transparent, and Robust Memory
Reclamation for Lock-Free Data Structures](https://arxiv.org/pdf/1905.07903.pdf)

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
