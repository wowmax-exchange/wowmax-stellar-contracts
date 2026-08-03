# Routing logic

This page explains what happens between "how much USDC for 100 XLM?" and the
unsigned transaction the wallet signs. It describes the observable contract of
the WOWMAX routing engine — inputs, guarantees, guards — at the interface
level. The optimizer itself (the pathfinding core WOWMAX has developed since
2022 across 20+ EVM chains) is proprietary and not covered here; everything a
user or integrator can rely on is.

## 1. The graph

The engine sees Stellar liquidity as one graph: tokens are nodes, liquidity
sources are edges. Two edge families exist side by side:

* **classic edges** — SDEX order books (price levels, deterministic fills);
* **soroban edges** — AMM pools on Soroswap, Phoenix and Aquarius (pricing
  curve, fee, pool reserves).

The graph is rebuilt from live chain state. By policy there is **no reserve
cache**: every quote reads current Horizon and Soroban RPC data, so a quote
can never be right about a pool that changed a minute ago. The quote response
reports the build (`graph.buildMs`, `classicEdges`, `sorobanEdges`) so the
freshness cost is visible rather than hidden.

A per-venue health snapshot is derived from every graph build (`/healthz`):
each venue is `up`, `degraded`, `down` or `idle` based on its read results.
A dead venue never blocks routing — its edges are simply absent and the
winner comes from the remaining liquidity. This is the graceful-degradation
path, and it is monitored and alerted on in production.

## 2. Two candidate routes, one winner

For a pair and an amount the engine computes the best route **twice** — once
in the classic world, once in the Soroban world — and returns the better
output. The response labels the winner with `mode`:

* `classic` — a `PathPaymentStrictSend` route. Deterministic: what SDEX
  quotes is exactly what executes.
* `soroban` — a multi-pool plan merged into one atomic contract invocation.
  Its true output is known by **simulation**, and the plan must fit Soroban
  resource budgets; a plan over budget is reduced or discarded rather than
  shipped broken.

Routes never mix the two worlds in one transaction. This rule costs a little
theoretical optimality and buys clean execution semantics — a trade the
engine makes deliberately.

## 3. Honest comparison: the advantage metric

Every quote also computes the best **single-pool** alternative for the pair
(best lone SDEX book, best lone AMM pool) and reports the routed advantage
over it in bps (`wowmax_advantage`). Three honest cases exist and the API
says which one you are in:

* the routed path genuinely beats every single pool — the advantage is the
  number shown;
* the optimal route *is* a single pool at this size — the engine returns it
  as-is instead of over-engineering a multi-hop path;
* no meaningful direct pool exists — routing is the only way to trade the
  pair at all, and the response says so.

## 4. Price guards between quote and execution

Two production guards protect the user from the gap between an analytical
quote and on-chain reality:

* **`rate_updated` guard.** Before building the transaction, the execution
  path re-derives the real output (for Soroban routes — by simulating the
  merged plan). If it lands more than **0.5%** below the quoted amount, the
  swap is *not* built; the client receives `rate_updated` with the real
  number, shows "Price updated — tap Swap to confirm", and only an explicit
  user confirmation proceeds at the new price.
* **Classic floor.** A Soroban plan whose simulated output falls below the
  deterministic classic route for the same pair is discarded in favour of
  classic. Simulation noise can never make the user execute worse than the
  order book guarantees.

On top of these, slippage protection is embedded in the transaction itself:
classic routes carry `destMin` (with a tight cap so the wallet's confirmation
screen shows a number close to the estimate, not a scary worst case), Soroban
routes carry the equivalent minimum-out constraint. If the chain moves past
the floor, the transaction fails atomically instead of filling badly.

## 5. Bridge ranking

Cross-chain quoting follows one principle: **every candidate on one axis,
with its reasons attached.**

1. Every wired bridge is quoted in parallel; a provider's refusal becomes a
   structured `noQuote` carrying the provider's own error text.
2. Composite routes (WOWMAX Stellar DEX leg + a bridge stable leg) are built
   for pairs where conversion-plus-bridge can beat any direct provider.
3. All candidates are normalised to **net USD after all fees** and ranked.
   When every priced option's gas cost is negligible, ranking falls back to
   raw destination-token output — bridges price with different feeds, and
   net-USD self-valuations can otherwise rank *more tokens below fewer*.
4. The winner is returned together with the full merged table, ETAs, and
   liquidity bounds (`maxAmountInUsd`) where providers expose depth.

The UI's instant first price is a deliberate two-phase trick: a fast
Near-only pass renders immediately, the full ranking follows in the
background and upgrades the displayed quote only if something beats it.

## 6. What this means for integrators

* Treat `mode` as informational — signing and broadcasting are identical for
  both; the XDR is always complete and unsigned.
* Always be ready for `rate_updated` on the swap-building step and re-confirm
  with the user; never auto-retry a worse price silently.
* Read `noQuotes` — "why a bridge declined" is often the product answer
  (unsupported pair, temporary liquidity gap, provider-side deprecation).
* Latency expectations are published and continuously measured: see the
  [public benchmark](D10-LATENCY.md) (DEX quote p95 ≈ 0.6 s on the public
  path; bridge fan-out p95 ≈ 4.6 s because it waits for the slowest
  provider).
