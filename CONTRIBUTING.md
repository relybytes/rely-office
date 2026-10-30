# Contributing

Thank you for helping. A few rules keep the project healthy and its licence stable.

## Developer Certificate of Origin

Every commit carries a `Signed-off-by:` line (`git commit -s`), stating that you wrote the change or
have the right to submit it under the project's licence, as described by the
[Developer Certificate of Origin 1.1](https://developercertificate.org/). There is no contributor
licence agreement: what you contribute is licensed under the Apache License 2.0, like the rest.

## Every source file

Starts with the licence header:

```rust
// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0
```

A new file you write may add your own copyright line under it.

## Before opening a pull request

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

- Inputs are untrusted: no `unwrap` or panic on bytes read from a document, every read bounded by
  `Limits`, no `unsafe` (the workspace forbids it).
- Tests build their documents in memory (`test_support.rs`); a binary fixture needs a reason in the
  pull request.
- A new dependency must have a licence `deny.toml` allows, and a reason.
- Code, comments, commits and docs are in English.
