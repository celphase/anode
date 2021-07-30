# Anode

Anode is a text editor plugin for [The Machinery] game engine.

- `tm-anode` - Anode text editor plugin for The Machinery.
- [![Crates.io](https://img.shields.io/crates/v/tm-anode-api.svg?label=tm-anode-api)](https://crates.io/crates/tm-anode-api) [![docs.rs](https://docs.rs/tm-anode-api/badge.svg)](https://docs.rs/tm-anode-api/) - API for the tm-anode The Machinery plugin.
- `tm-textfile` - Text file plugin for The Machinery that showcases basic usage of `tm-anode-api`.

[the machinery]: https://ourmachinery.com/

## Building and Installing

Anode is built using the [Cargo] package manager for [Rust], and can be automatically copied to your
`TM_SDK_DIR` plugins directory using a [cargo-make] task.

```
cargo make machinery
```

[cargo]: https://doc.rust-lang.org/cargo/
[rust]: https://www.rust-lang.org/
[cargo-make]: https://github.com/sagiegurari/cargo-make

## Extending

Anode provides a public API that can be used to extend it.
The API is documented in the `tm-anode-api` crate, and is compatible with the C ABI.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License (Expat) ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
