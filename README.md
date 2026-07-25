# StreamFi Contracts

Soroban smart contracts that power on-chain features of [StreamFi](https://streamfi.media) — a Stellar-native live streaming platform.

## Status

Early. First contract is `subscription`, which handles per-creator subscription tiers, one-time time-bounded purchases, and access checks that the StreamFi backend calls before issuing signed Mux playback tokens.

## Repository layout

```
streamfi-contracts/
├── contracts/
│   └── subscription/       # Per-creator subscription escrow
│       ├── src/
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── docs/                   # Design notes and roadmap
├── Cargo.toml              # Workspace root
├── rust-toolchain.toml     # Pinned toolchain
└── .github/                # CI + issue templates
```

## Requirements

- **Rust** (`stable` toolchain — pinned in `rust-toolchain.toml`)
- **Stellar CLI** (`stellar` — [install](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup))
- **wasm32 target**: `rustup target add wasm32-unknown-unknown`

## Quick start

```bash
# Build every contract to WASM
cargo build --release --target wasm32-unknown-unknown

# Run the full test suite
cargo test

# Format and lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

The built WASM artifacts land in `target/wasm32-unknown-unknown/release/`.

## Deploying (testnet example)

```bash
# Build
cargo build --release --target wasm32-unknown-unknown

# Deploy the subscription contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/streamfi_subscription.wasm \
  --source alice \
  --network testnet

# Initialize with admin + payment token (USDC on testnet)
stellar contract invoke \
  --id <contract_id> \
  --source alice \
  --network testnet \
  -- init \
  --admin <admin_address> \
  --payment_token <usdc_contract_address>
```

## Contracts

| Contract | Purpose | Status |
|----------|---------|--------|
| `subscription` | Per-creator subscription tiers + access checks | ✅ MVP |
| `tip-goal-escrow` | Kickstarter-style tip goals with auto-refund | ⏳ Planned |
| `revenue-split` | Atomic tip split between multi-host streams | ⏳ Planned |
| `token-gated-access` | Membership-token gate for private streams | ⏳ Planned |
| `payout-scheduler` | Automated creator payouts on cadence | ⏳ Planned |

See [docs/roadmap.md](docs/roadmap.md) for details.

## Integration with StreamFi

The backend (see [streamfi-frontend](https://github.com/StreamFi-x/streamfi-frontend)) calls the contract's read-only `is_active(subscriber, creator)` before issuing a signed Mux playback token for subscribers-only streams. This gives us CDN-level access enforcement backed by on-chain state.

## Contributing

We welcome contributions. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, coding standards, and PR conventions. Security issues should be reported per [SECURITY.md](SECURITY.md).

## License

Apache-2.0 — see [LICENSE](LICENSE).
