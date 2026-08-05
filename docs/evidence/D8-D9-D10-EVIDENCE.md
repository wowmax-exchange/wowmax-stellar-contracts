# SCF Build Award #43 — Evidence Package: Deliverables 8, 9, 10

Prepared 2026-08-03. All links public unless marked. Placeholders to fill
before submission are marked `[FILL]`.

---

## Deliverable 8 — Monitoring & Production Operations

**Criterion: Grafana live for all DEXes and bridges — DONE.**

Two dashboards, publicly viewable without login (read-only, single-dashboard
scope):

* Router & DEX venues (SDEX, Soroswap, Phoenix, Aquarius — per-venue health,
  quote success, winner mode, latency p50/p95, RPC-pool internals):
  https://grafana.wowmax.exchange/public-dashboards/ec1ed488b13843e1a11c1e0346864bd3
* Bridges (Near Intents, Allbridge, Axelar, Squid + composite routes —
  per-bridge availability per pair, winner, net USD, ETA, fan-out latency):
  https://grafana.wowmax.exchange/public-dashboards/9132626551e94f5b8af0ba54ff0e1946

Branded landing with both dashboards: https://grafana.wowmax.exchange

Backing infrastructure: synthetic prober (systemd service) exercising real
quotes across every venue and bridge on a 90/180/120-second cadence, exposing
Prometheus metrics (including true latency histograms) into a dedicated
VictoriaMetrics instance (180-day retention), consumed by Grafana over an
authenticated tunnel-exposed datasource.

**Criterion: Alerts tested successfully — DONE.**

Eight provisioned rules (router down; per-venue not serving; per-pair DEX
probe failing; bridge service down; per-pair no direct bridge; public
gateway e2e down; router p95 breach; prober down as the single blindness
sentinel), evaluated every 60 s, routed via a `team=stellar` notification
policy to Telegram with a compact two-line message template.

Live validations during the build (not synthetic):

* p95 rule fired on a real latency breach (15.6 s p95 on heavy probe pairs),
  leading to a threshold retune to 20 s per pair — alert screenshot retained.
* Controlled sentinel test: prober stopped → firing delivered → restarted →
  resolved delivered.
* A datasource blip (edge-tunnel reconnect, HTTP 502) initially produced an
  alert storm; rules were hardened (`noDataState=OK`, `execErrState=OK` for
  r1–r7) so any monitoring-path failure now produces exactly one sentinel
  alert.

**Criterion: Runbook published with 6+ incident scenarios — DONE (8).**

https://github.com/wowmax-exchange/wowmax-stellar-contracts/blob/master/docs/RUNBOOK.md
— commits `e452d35` (initial, 8 scenarios) and `41a81ba` (system-map
correction after tracing the public edge). Two scenarios are written from
real incidents handled during the build:

* Near Intents transient "No liquidity available" (2026-08-02 21:03 CEST,
  provider correlationId retained) — recovered unaided; anti-flap thresholds
  correctly stayed silent.
* Allbridge suspending its Stellar routes (provider-side protocol
  deprecation, verified against their SDK 3.32.0 and 3.32.1 with a full
  messenger matrix) — adapter now declines SRB legs cleanly behind a
  re-enable flag.

---

## Deliverable 9 — Documentation & Developer Integration Guide

**Criterion: Public documentation site live — [FILL: Pages URL].**

Content published in-repo (commits `26960d1`, `b4734fc`):
https://github.com/wowmax-exchange/wowmax-stellar-contracts/tree/master/docs

* `README.md` — index and live-surface directory
* `ARCHITECTURE.md` — components, data flow, the two Stellar execution
  layers, cross-chain model, non-custodial guarantees
* `ROUTING.md` — routing logic at the interface level: graph, winner
  selection, advantage metric, `rate_updated` and classic-floor price guards,
  bridge ranking
* `USER-GUIDE.md` — swapping and bridging walkthrough
* `INTEGRATION.md` — developer quickstart
* `CONTRIBUTING.md` — contribution model incl. how a new bridge or venue
  joins the ranking and the monitoring matrix

GitHub Pages URL once enabled (Settings → Pages → master + /docs):
https://wowmax-exchange.github.io/wowmax-stellar-contracts/ — live, verified 2026-08-03.

**Criterion: Developer quickstart under 15 minutes — DONE.**

`docs/INTEGRATION.md`: first live quote in ≈3 minutes (npm i + 10 lines),
unsigned swap XDR in ≈3 more, optional real broadcast, cross-chain quote in
the same client. No API key and no funds required except the optional
broadcast step.

**Criterion: Public post-mortem blog published — [FILL: Medium URL].**

Article prepared ("WOWMAX on Stellar: What We Built vs. What We Planned"):
honest roadmap deltas (pre-award re-scope to the aggregation track; Squid as
the fourth bridge; composite routes as an unplanned innovation; the Allbridge
suspension as a live validation of the aggregation thesis), published
production numbers, and lessons. Venue: medium.com/wowmax-exchange →
[FILL URL after publishing].

---

## Deliverable 10 — Open Aggregation API for the Stellar Ecosystem

**Criterion: API live — DONE.**

Public gateway: https://api-gateway.wowmax.exchange — Swagger UI at
[/docs](https://api-gateway.wowmax.exchange/docs). (The award text's
shorthand `api.wowmax.exchange` refers to this canonical host per the
tracked deliverable definition.)

**Criterion: OpenAPI specification published — DONE.**

https://api-gateway.wowmax.exchange/docs-json — includes the DEX surface and
all 9 cross-chain endpoints under `/crosschain/v0/bridge/*` (chains, tokens,
routes, quote, quote-compat, execute, status, squid/status,
allbridge/status).

**Criterion: TypeScript SDK published to npm — DONE.**

https://www.npmjs.com/package/@wowmax/sdk — `@wowmax/sdk@0.2.0`
(`latest`). 0.1.0 shipped the DEX cycle; 0.2.0 adds the full bridge cycle
(`bridgeChains/Tokens/Routes/Quote/QuoteCompat/Execute/Status`), typed
responses with liquidity-depth passthrough, and provider-aware 60 s timeouts
for fan-out calls. Source: https://github.com/wowmax-exchange/wowmax-sdk (tags
`631f9fe` → `e02733f`).

**Criterion: Quickstart under 10 minutes — DONE.**

Package README (npm page) reaches a first quote in ≈3 minutes; the full
docs-site quickstart covers the complete non-custodial cycle.

**Criterion: p95 latency benchmark published — DONE.**

https://github.com/wowmax-exchange/wowmax-stellar-contracts/blob/master/docs/D10-LATENCY.md
(commit `8154a32`, raw timings attached as `bench_p95.json`):

| Public endpoint | n | p50 | p95 | p99 |
|---|---:|---:|---:|---:|
| DEX quote (XLM→USDC, live reserves) | 200 | 0.383 s | 0.611 s | 1.193 s |
| Bridge discovery | 100 | 0.066 s | 0.085 s | 0.089 s |
| Bridge quote fan-out (waits for slowest provider) | 25 | 1.525 s | 4.568 s | 5.863 s |

Continuously re-measured in production (see the public dashboards above).

**Criterion: First external integration documented — [FILL].**

`docs/INTEGRATIONS.md` pending two facts (integrator identity/anonymity
preference and current status) from the July 17 external integration case.

---

## Screenshot checklist for submission

1. Both public dashboards open in an incognito window (data visible, no
   login) — one screenshot each.
2. Telegram: one 🔴 firing + one ✅ resolved in the compact format.
3. Bridges dashboard zoomed to 2026-08-02 ~21:00 CEST showing the
   Near Intents availability dip (monitoring catching a real incident).
4. Swagger UI with the `/crosschain/v0/bridge/*` section expanded.
5. npm page showing `@wowmax/sdk 0.2.0`.
6. GitHub Pages docs index rendered.
7. Medium post live.
