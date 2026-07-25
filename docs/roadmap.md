# Contract Roadmap

The contracts that ship in the initial cut of StreamFi's on-chain layer, and what comes next.

## Shipped

### `subscription` ✅

Per-creator subscription tiers with one-time time-bounded purchases. Payment via a Stellar Asset Contract (USDC). Read-only `is_active(subscriber, creator)` for the backend to gate signed Mux playback tokens on subscribers-only streams.

### `tip-goal-escrow` ✅

Kickstarter-style tip goals. Creator sets a goal (amount, deadline). Tippers contribute into the contract (funds held in escrow at the contract address). If the goal is met by the deadline, `claim()` releases funds to the creator. If not, each tipper calls `refund()` to withdraw their contribution.

Why it matters: gives tippers real confidence that a big tip toward a specific outcome (new mic, new PC, charity drive) is not just going to disappear.

### `revenue-split` ✅

Multi-recipient atomic tip split by basis points. When a stream has co-hosts, each tip lands via `pay()` and gets atomically distributed between recipients (up to 10) with a bps configuration that must sum to exactly 10 000. Rounding remainder goes to the last recipient — no lost dust. No manual reconciliation, no "one person collects and Venmos everyone."

### `token-gated-access` ✅

Per-creator paid "access gates" for exclusive content. Viewers `purchase()` a gate — the fee transfers directly to the creator and grants a permanent (`duration_secs = 0`) or time-bounded access record. The backend calls read-only `has_access(gate_id, holder)` before minting a signed Mux JWT. Creators can `revoke()` if needed.

### `payout-scheduler` ✅

Creators configure a payout schedule (destination, minimum balance, cadence). External payments flow into a pool via `deposit()`. Anyone can `trigger_payout()` once the cadence has elapsed and the pool is at or above the minimum — the full pool sweeps to the destination and the cadence resets. Reduces manual payout ops and lets bots/keepers automate sweeps.

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
