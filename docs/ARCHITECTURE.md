# Architecture

This page describes how the WOWMAX Stellar stack is put together: which
services exist, how a request travels through them, how the two Stellar
execution layers are handled, and why the system is non-custodial by
construction.

## 1. Components

```
                        ┌──────────────────────────────┐
                        │        User / Integrator      │
                        │  app.wowmax.exchange · SDK ·  │
                        │  REST · MCP tools             │
                        └──────────────┬───────────────┘
                                       │ HTTPS (Cloudflare)
                        ┌──────────────▼───────────────┐
                        │   api-gateway (Kubernetes)    │
                        │   NestJS · Swagger /docs      │
                        │   /chains/100000148/*         │
                        │   /crosschain/v0/bridge/*     │
                        └──────────────┬───────────────┘
                                       │ HTTPS
                        ┌──────────────▼───────────────┐
                        │   Caddy edge (routing host)   │
                        │   path split:                 │
                        │   /bridge/*  → :8085          │
                        │   everything → :8083          │
                        └───────┬──────────────┬───────┘
                                │              │
                 ┌──────────────▼───┐   ┌──────▼──────────────┐
                 │ Stellar router v2│   │  bridge-aggregator   │
                 │ (:8083)          │   │  (:8085)             │
                 │ SDEX · Soroswap  │   │  Near Intents ·      │
                 │ Phoenix · Aqua   │   │  Allbridge · Axelar ·│
                 │ quote + swap XDR │   │  Squid · composites  │
                 └───────┬──────────┘   └──────┬───────────────┘
                         │                     │
              ┌──────────▼─────────┐   ┌──────▼───────────────┐
              │ Horizon + Soroban  │   │ Bridge provider APIs │
              │ RPC pool (multi-   │   │ + router v2 for the  │
              │ provider fallback) │   │ composite Stellar leg│
              └────────────────────┘   └──────────────────────┘
```

**Stellar router v2** computes DEX quotes across four venues and builds the
unsigned swap transaction. It maintains a routing graph over classic SDEX
order books and Soroban AMM pools, refreshed live — by policy there is no
reserve cache: every quote reads current Horizon and Soroban state.

**bridge-aggregator** quotes every wired provider in parallel — Near Intents
(through the Aurora Intents gateway), Squid, which is also the path Axelar's
ITS Hub takes to Stellar, and Allbridge — folds in composite routes (a bridge
leg plus a WOWMAX DEX leg on Stellar), ranks all candidates on one net-USD
axis, and can produce the unsigned execution payload for the winning route.
Adapters are scoped to the chains where they genuinely serve routes, so the
ranking lists real options instead of the same path under two names. One misbehaving provider SDK cannot take the
service down: provider calls are isolated and a refusal is reported as a
structured `noQuote` with the provider's own reason.

**api-gateway** is the single public API surface. It exposes the chain-keyed
DEX endpoints and thin-proxies the bridge API under `/crosschain/v0/bridge/*`,
publishes the OpenAPI document, and serves the Swagger UI at `/docs`.

**Caddy** terminates TLS on the routing host and splits one hostname by path
between the two local services, so both stay bound to localhost while the
gateway (and only the gateway path) consumes them.

**Monitoring** — a synthetic prober exercises real quotes across every venue
and every bridge on a fixed cadence, exports Prometheus metrics (including
per-venue health straight from the router and latency histograms), feeds
Grafana dashboards, and drives an alert set with Telegram delivery. The
operational side is documented in the [runbook](RUNBOOK.md).

## 2. The two Stellar execution layers

Stellar has two incompatible execution models, and the router treats them as
parallel worlds that never mix inside one transaction:

* **Classic** — the native SDEX order books, executed with
  `PathPaymentStrictSend`. Multi-hop paths are atomic at the protocol level
  and deterministic: the quoted amount is the executed amount.
* **Soroban** — smart-contract AMMs (Soroswap, Phoenix, Aquarius), executed
  through the open-source WOWMAX execution contracts in this repository,
  which merge a multi-pool plan into one atomic invocation under Soroban
  resource budgets.

For every quote the engine computes the best route in each world and returns
the better of the two, labelled with its `mode` (`classic` or `soroban`). The
rule *no route mixes classic and Soroban operations* keeps execution
semantics clean: deterministic order-book math on one side, simulated
contract execution on the other. [Routing logic](ROUTING.md) covers how the
winner is chosen and which price guards protect the user between quote and
execution.

## 3. Cross-chain model

The bridge aggregator normalises very different providers into one
comparison: pool-based bridges, intent/RFQ systems and message-passing
bridges all reduce to *net output after all fees, in USD, with an ETA and a
liquidity bound where the provider exposes one*. Two route kinds compete:

* **Direct** — one provider carries the transfer end to end.
* **Composite** — the WOWMAX Stellar DEX leg converts the asset on Stellar,
  then a bridge carries a stable leg (or vice versa). Composites exist
  because the best route for `XLM → USDT (BSC)` is often *not* a single
  bridge.

Providers that decline a pair stay visible in the response (`noQuotes`, with
the provider's reason) — the ranking never hides why an option is absent.

## 4. Non-custodial guarantees

No service in this stack ever holds keys, signs, or broadcasts on the user's
behalf:

* DEX swaps: the router returns an **unsigned XDR** (classic path payment or
  Soroban invocation). The wallet signs locally; the client broadcasts.
* Bridge transfers: the aggregator returns **unsigned raw transactions,
  unsigned XDR, or deposit instructions**, depending on the provider's model.
  Approvals on EVM legs are built as separate optional transactions.
* The SDK (`@wowmax/sdk`) is a typed HTTP client over these same endpoints —
  it contains no signing code at all.

The Soroban execution contracts are open source in this repository and were
tagged for an independent security review (`audit-2026-07`).

## 5. Networks

Everything above runs on Stellar **mainnet**. The router additionally serves
**testnet** (`?network=testnet`) with the assets that exist there (currently
`XLM` and the `WUSD` test asset) — used for development sandboxes. The
[developer quickstart](INTEGRATION.md) runs against mainnet: quoting and
building unsigned transactions are free, so no test funds are needed.
