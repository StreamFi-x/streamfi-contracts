# Contract Roadmap

The contracts we want to build, in rough priority order. Grab one and open a PR.

## Shipped

### `subscription` ✅

Per-creator subscription tiers with one-time time-bounded purchases. Payment via a Stellar Asset Contract (USDC). Read-only `is_active(subscriber, creator)` for the backend to gate signed Mux playback tokens on subscribers-only streams.

## Next up

### `tip-goal-escrow`

Kickstarter-style tip goals. Creator sets a goal (amount, deadline). Tippers contribute into the contract. If the goal is met by the deadline, funds release to the creator. If not, tippers can withdraw their contribution.

Why it matters: gives tippers real confidence that a big tip toward a specific outcome (new mic, new PC, charity drive) is not just going to disappear.

### `revenue-split`

Multi-host tip split. When a stream has co-hosts, tips land in the contract and get atomically split between hosts by a preconfigured percentage. No manual reconciliation, no "one person collects and Venmos everyone."

### `token-gated-access`

Mint a Stellar asset representing membership. Contract exposes `has_access(holder)` that the StreamFi backend checks before minting a signed Mux JWT. Ties on-chain ownership to real-world stream access.

### `payout-scheduler`

Creator sets rules (min balance, cadence). The contract auto-sweeps accumulated tips to the creator's wallet on schedule. Reduces manual payout ops.

## Later

- `nft-subscription-cert` — each active sub is a transferable Stellar NFT
- `streamer-verification-bond` — creator locks a small XLM bond, returned after N successful streams
- `channel-points-token` — per-creator loyalty currency as a real asset
- `gift-sub-community-pool` — anyone contributes, contract atomically distributes gift subs to next N viewers
- `delayed-tip-refund-window` — tips escrowed for N hours; tipper can revoke, otherwise released

## Design principles

- **Each contract lives in its own folder**, no shared helper crate. Makes them independently auditable and mergeable in any order.
- **Payments are always in a specific asset** (typically USDC via the Stellar Asset Contract) — no ambiguity about denomination.
- **Read-only accessors are cheap and complete** — the StreamFi backend calls them frequently, so they should not require auth and should return everything the backend needs in one call.
- **Events are first-class** — every state-changing call publishes an event. This is what feeds indexers and the frontend's live UI updates.
- **Errors use `#[contracterror]`**, never `panic!` with strings. Callers should be able to switch on discriminants.
