# wowmax-stellar-contracts

> Soroban smart contracts for the [WOWMAX](https://wowmax.exchange) DEX
> aggregator on Stellar — the on-chain **execution layer**.

Part of the WOWMAX deliverable for the
[Stellar Community Fund (SCF) Build Award](https://communityfund.stellar.org/),
Integration Track.

## What this is

This repository contains the **on-chain execution contract** for WOWMAX on
Stellar: a Soroban contract that executes a pre-computed swap plan —
splits, multi-hop paths, and **cross-protocol** routes — **atomically in a
single transaction** across Stellar's Soroban DEX protocols.

The contract is a **plan executor**, not a router. Route selection (which
pools, which splits, in what proportion) is computed off-chain by the
WOWMAX pathfinder and handed to the contract as an explicit plan; the
contract executes exactly that plan and enforces end-to-end slippage. This
separation is deliberate: the contract holds no routing logic, so it can be
audited as a thin, predictable executor.

### What it does that a per-protocol aggregator cannot

The common Soroban aggregator pattern dispatches each leg of a split to a
single protocol (one protocol per path). This contract goes further:

- **Cross-protocol multi-hop in one call** — e.g. AQUA→USDC on Aquarius,
  then USDC→XLM on Soroswap, executed as one atomic `InvokeHostFunction`.
  The intermediate token is held by the contract between hops and fed
  forward by its actual balance.
- **Exact-pool targeting** — the plan names the precise pool for each leg
  (Aquarius `pool_index`, Soroswap pair, Phoenix pool), rather than
  delegating pool choice to a protocol's own router.
- **Atomic slippage on the summed output** — `amount_out_min` is enforced
  on the total across all split branches; any failing branch reverts the
  whole transaction.

## Supported venues

| Venue | Type | Status |
|---|---|---|
| Soroswap | UniswapV2-style AMM (Soroban) | live |
| Aquarius | StableSwap / weighted AMM (Soroban) | live |
| Phoenix | XYK AMM (Soroban) | live |
| Comet | Comet pool AMM (Soroban) | adapter only — not deployed |

The Comet adapter (`contracts/adapters/comet`) is implemented and
unit-tested but is not wired into the deployed executors.

Classic SDEX (Stellar's native order book) is **not** executed by this
contract — SDEX is path-payment-based, not a contract call, and is handled
in the classic execution path outside this repository. This contract covers
the three Soroban DEX protocols.

## Contract interface

The contract (crate `wowmax-stellar-router`, struct `WowmaxAggregator`)
exposes:

| Function | Purpose |
|---|---|
| `swap(user, token_in, token_out, amount_in, amount_out_min, deadline, plan)` | **Main entry.** Executes a `Vec<Strand>` plan: parallel splits, each with sequential hops, across any mix of the three venues. Slippage enforced on the summed output. |
| `swap_soroswap(...)` | Single Soroswap swap along a path. |
| `swap_aqua(...)` | Single Aquarius `swap_chained` hop on a named pool. |
| `swap_phoenix(...)` | Single Phoenix pool swap. |
| `swap_aqua_then_soroswap(...)` | Cross-protocol two-leg helper (Aquarius → Soroswap). |
| `swap_merge(...)` | Pooled-merge execution: runs the route as topologically-ordered stages, splitting each token's pooled balance once across its pools (on-chain fan-in). One swap per graph edge keeps heavy multi-branch routes within the Soroban instruction budget. |

### Plan model

```
plan: Vec<Strand>
Strand { parts: u32, hops: Vec<Hop> }
Hop    { venue, pool, token_in, token_out, <venue-specific fields> }
```

The contract splits `amount_in` across strands by integer `parts`
(`strand_in = floor(amount_in * parts / total_parts)`, the last strand
taking the remainder), runs each strand's hops sequentially (each hop
consuming the previous hop's output), sums the strand outputs, checks
`amount_out_min`, and forwards the proceeds to `user`.

## Mainnet validation

All functions are validated on **Stellar mainnet** against live pools. See
[`docs/REPORT.md`](./docs/REPORT.md) for the full report and
[`docs/evidence/mainnet-tx.md`](./docs/evidence/mainnet-tx.md) for the
transaction list with explorer links.

Deployed contract (mainnet):
[`CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF`](https://stellar.expert/explorer/public/contract/CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF),
wasm sha256 `095ee35248f9076fb76d26d7d97e1308c35586df364ff0442b664c5fb3718883`.
A single deployment now carries every entry point including `swap_merge`;
the two contracts it replaces are recorded under `superseded` in
[`public/mainnet.contracts.json`](./public/mainnet.contracts.json). The
contract has **no admin and no upgrade path** — replacement is by
redeployment.

## Safety properties

- Output is measured as the contract's **own `token_out` balance delta**,
  never the figure a venue reports, so an inflated return cannot make the
  forward transfer exceed the contract's balance.
- `amount_out_min` is enforced on the **summed output across all strands**;
  any failing branch reverts the whole transaction.
- Token continuity is validated: a strand must start at `token_in`, each
  hop must consume the previous hop's `token_out`, and the last hop must
  produce `token_out`.
- `deadline` is enforced on-chain, not delegated to the venue.
- Amounts are validated before any cast or transfer; self-swaps and
  degenerate hops are rejected.
- No funds are retained: after execution the contract's balances of the
  tokens involved are zero.

## Building and testing

```bash
cd contracts
make test          # stellar contract build, then cargo test --workspace
```

Or step by step:

```bash
cd contracts
stellar contract build              # -> target/wasm32v1-none/release/wowmax_*.wasm
cargo test --workspace              # every suite
cargo test -p wowmax-stellar-router # the executor only
```

The workspace must be built before any test suite is compiled: the adapter
suites import the `.wasm` artifacts of other crates.

The executor test suite covers the splitter arithmetic and remainder
handling, the slippage guard, token continuity, input validation, and a
regression in which a venue reports a larger output than it delivers. It
also reports the CPU budget consumed by a deliberately heavy plan
(`cargo test -- --nocapture`).

Requires `stellar` CLI 26+ and the `wasm32v1-none` target. Produces
`contracts/target/wasm32v1-none/release/wowmax_stellar_router.wasm`.

## Security

Threat model and data flow diagram: [`docs/security/`](./docs/security/).
Vulnerability disclosure: [SECURITY.md](./SECURITY.md).

## About WOWMAX

[WOWMAX](https://wowmax.exchange) is a non-custodial DEX aggregator and
copy-trading platform: $2B+ cumulative swap volume, 336K+ traders, 20+ EVM
chains. The Stellar integration adds the first non-EVM chain to the network.

## License

[GPL-3.0-only](https://spdx.org/licenses/GPL-3.0-only.html). See
[LICENSE](./LICENSE). This repository began as a derivative of an
Apache-2.0 project; the upstream license text is preserved in
[LICENSE.Apache-2.0](./LICENSE.Apache-2.0) and attribution in
[NOTICE](./NOTICE).
