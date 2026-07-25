# Contributing to StreamFi Contracts

Thanks for your interest in contributing! This repo holds Soroban smart contracts that power StreamFi's on-chain features. Contributions of any size are welcome — bug reports, docs improvements, new tests, or full contract implementations.

## Ground rules

- **Every new contract lives in its own folder under `contracts/`.** No shared helper crate unless a genuine need appears — keeping contracts isolated makes them easier to audit and mergeable in any order.
- **Tests are required.** No contract merges without unit tests that exercise the happy path and the main error paths.
- **Fmt and clippy must pass.** CI enforces this.
- **Follow existing patterns.** Look at `contracts/subscription/` for the conventions we use (error enums via `#[contracterror]`, events via `#[contractevent]`, storage keys as a `DataKey` enum, `require_auth()` on any state-changing call).

## Development setup

1. Install [Rust](https://rustup.rs/) — the `stable` toolchain is pinned in `rust-toolchain.toml`.
2. Add the WASM target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```
3. Install the [Stellar CLI](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup):
   ```bash
   cargo install --locked stellar-cli --features opt
   ```
4. Verify:
   ```bash
   cargo test              # runs all contract test suites
   cargo build --release --target wasm32-unknown-unknown
   ```

## Adding a new contract

1. Pick an item from [docs/roadmap.md](docs/roadmap.md), or open an issue proposing a new one first.
2. Scaffold a new folder:
   ```
   contracts/<your-contract>/
     Cargo.toml
     src/
       lib.rs
       test.rs
   ```
3. In `Cargo.toml` use the workspace inheritance already used by `contracts/subscription/`:
   ```toml
   [package]
   name = "streamfi-<your-contract>"
   version = { workspace = true }
   edition = { workspace = true }
   license = { workspace = true }
   repository = { workspace = true }
   description = "One-line description."

   [lib]
   crate-type = ["cdylib", "rlib"]
   ```
4. Write the contract, publish events, cover errors with `#[contracterror]`, and add tests.
5. Run `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test` locally before opening a PR.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(subscription): add pause-and-resume
fix(subscription): correct expiry math on same-day resubscribe
docs(readme): clarify integration example
chore(ci): pin cargo-audit
```

Keep the subject under 72 chars.

## Pull requests

- Target `main`.
- One contract change per PR when possible. Cross-contract refactors are OK but should be called out clearly.
- Fill in the PR template.
- Include a summary of what changed and why, plus test evidence (paste `cargo test` output if adding a new contract).
- CI must be green before review.

## Security

Do **not** open a public issue for security vulnerabilities. See [SECURITY.md](SECURITY.md) for the disclosure process.

## Code of Conduct

Be kind. Be direct. Attack ideas, not people. Assume good faith. If a problem comes up, contact the maintainers.

## License

By contributing, you agree that your contributions will be licensed under Apache-2.0 (see [LICENSE](LICENSE)).
