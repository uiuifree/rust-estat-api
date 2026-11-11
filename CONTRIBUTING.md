# Contributing

Bug reports and pull requests are welcome on
[GitHub](https://github.com/uiuifree/rust-estat-api).

Run the same checks as CI before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --doc
```

The minimum supported Rust version is declared in `Cargo.toml` (`rust-version`) and
checked in CI.

## Coverage

All lines are covered by tests. Check with
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov):

```bash
cargo llvm-cov --all-targets --all-features --summary-only
```

## Live tests

`tests/live.rs` calls the real API and is ignored by default. Run it with an application ID, one
test at a time to keep the request rate low:

```bash
ESTAT_APP_ID=... cargo test --test live -- --ignored --test-threads 1
```

In CI it runs on pushes to `main`, weekly, and on manual runs when the repository secret
`ESTAT_APP_ID` is set.
