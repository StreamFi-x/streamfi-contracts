# StreamFi Contracts

Soroban smart contracts that power on-chain features of [StreamFi](https://streamfi.media) — a Stellar-native live streaming platform.

## Status

Five contracts shipped covering the initial on-chain surface: subscriptions, tip goals with escrow, atomic revenue splits, paid access gates, and scheduled payouts. All contracts are independently deployable and share no runtime dependencies.

## Repository layout

```
streamfi-contracts/
├── contracts/
│   ├── subscription/         # Per-creator subscription tiers
│   ├── tip-goal-escrow/      # Kickstarter-style tip goals
│   ├── revenue-split/        # Atomic multi-recipient splits
│   ├── token-gated-access/   # Paid access gates for exclusive content
│   └── payout-scheduler/     # Cadence-based auto-sweep payouts
├── docs/                     # Design notes and roadmap
├── Cargo.toml                # Workspace root
├── rust-toolchain.toml       # Pinned toolchain
└── .github/                  # CI + issue templates
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
| `subscription` | Per-creator subscription tiers + access checks | ✅ Shipped |
| `tip-goal-escrow` | Kickstarter-style tip goals with escrow + refund | ✅ Shipped |
| `revenue-split` | Atomic multi-recipient split by basis points | ✅ Shipped |
| `token-gated-access` | Paid access gates (permanent or time-bounded) | ✅ Shipped |
| `payout-scheduler` | Cadence-based auto-sweep from creator pool | ✅ Shipped |

See [docs/roadmap.md](docs/roadmap.md) for details.

## Integration with StreamFi

The backend (see [streamfi-frontend](https://github.com/StreamFi-x/streamfi-frontend)) is designed to call these contracts' cheap read-only accessors — `subscription.is_active(subscriber, creator)` and `token_gated_access.has_access(gate_id, holder)` — before minting signed Mux playback tokens for gated content. That gives us CDN-level access enforcement backed by on-chain state, with no periodic reconciliation job needed.

Payment flows (`tip-goal-escrow.contribute`, `revenue-split.pay`, `payout-scheduler.deposit`) are called directly by user wallets or by keeper bots, with the frontend surfacing events published by each contract for live UI updates.

## Contributing

We welcome contributions. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, coding standards, and PR conventions. Security issues should be reported per [SECURITY.md](SECURITY.md).

## License

Apache-2.0 — see [LICENSE](LICENSE).
