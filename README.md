# classified_macros

`#[derive(Classified)]` for the [`classified`](https://crates.io/crates/classified)
crate. Use it through `classified` with the `derive` feature:

```toml
[dependencies]
classified = { version = "0.2", features = ["derive"] }
```

```rust
use classified::Classified;

#[derive(Classified)]
pub struct Wallet {
    seed: [u8; 32],
    passphrase: String,
    #[classified(public)]
    label: String,
}

let wallet = Wallet { seed: [7; 32], passphrase: "hunter2".into(), label: "cold".into() };

// secret fields are redacted, public ones are shown
assert_eq!(format!("{wallet:?}"), r#"Wallet { seed: [REDACTED], passphrase: [REDACTED], label: "cold" }"#);

// every field is reachable inside `expose`, through a generated `WalletView<'_>`
let n = wallet.expose(|view| view.seed.len() + view.passphrase.len());
assert_eq!(n, 39);

// `seed` and `passphrase` are zeroized when `wallet` drops
```

The derive generates a `Drop` that zeroizes every secret field (each field
type must implement `zeroize::Zeroize`), a `Debug` that prints them as
`[REDACTED]`, a `<Name>View<'view>` struct borrowing each field, an inherent
`expose`, and an impl of `classified::Expose`. A field marked
`#[classified(public)]` is printed as usual and left alone on drop.

Generic structs are supported; put the `Zeroize` bound on the struct's own
generics, since a `Drop` impl cannot add bounds the struct does not have.

## License

MIT OR Apache-2.0.
