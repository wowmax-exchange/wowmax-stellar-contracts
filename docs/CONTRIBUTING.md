# Contribution model

This page explains what is open, where contributions land, and how to add
support for a new DEX venue or a new bridge to the WOWMAX Stellar stack.

## 1. What is open, what is not

| Repository | Status | What lives there |
| --- | --- | --- |
| `wowmax-stellar-contracts` (this repo) | **open (MIT)** | Soroban execution contracts, public documentation, operations runbook, benchmark reports |
| `bridge-aggregator` | **open (MIT)** | The cross-chain aggregation service: provider adapters, composite routing, ranking, status tracking |
| `wowmax-sdk` | **open (MIT)** | The TypeScript client published as `@wowmax/sdk` |
| `wowmax-benchmarks` | **open** | The benchmark harness behind the published numbers |
| Routing engine core | private | The pathfinding optimizer WOWMAX has developed since 2022. Its observable behaviour is fully documented in [Routing logic](ROUTING.md); the optimizer internals are not open source |

Everything a third party needs to *use* the stack — API, SDK, contracts,
docs — is public. The one private component is behind a documented interface,
and its execution side (the Soroban contracts your funds actually touch) is
open and was tagged for independent security review (`audit-2026-07`).

## 2. Adding a bridge

Bridges are self-contained adapters in `bridge-aggregator`. An adapter
implements one interface and the aggregator does the rest — parallel
quoting, ranking, composites, monitoring:

* `name` — the bridge id shown in rankings and statuses;
* `supports(request)` — a fast, local answer: can this provider in principle
  serve the chain pair? Unsupported must be cheap and honest (see how the
  Allbridge adapter declines Stellar legs while the provider has them
  suspended — a clean `unsupported route` beats a runtime 400);
* `quote(request)` — one live quote, or a thrown error with the provider's
  own reason (it becomes a structured `noQuote`, never a crash);
* `execute(request)` — the **unsigned** payload: raw transactions, XDR, or
  deposit instructions. Adapters never sign;
* `status(id)` — map the provider's lifecycle onto
  `pending | success | failed | refunded`.

Ground rules learned in production: isolate every provider SDK call (one
misbehaving SDK must not take the service down); keep provider config in env
(`CC_<BRIDGE>_*`) with the adapter disabled unless configured; surface the
provider's own error text — the ranking's honesty depends on it.

A new adapter PR should come with: the env vars documented, a quote+status
happy path demonstrated against the provider's testnet or a small mainnet
amount, and a probe pair suggestion for the monitoring matrix so the new
bridge is alerted on from day one.

## 3. Adding a DEX venue

Venue integrations (a new AMM or order-book source on Stellar) touch the
private routing core, so they land differently: open an issue describing the
venue — contracts or Horizon surface to read liquidity from, how pricing is
computed, expected depth — and the team wires the loader against the
documented venue contract: read pools/books into graph edges, report
per-venue health (`ok / noPool / error`) into `/healthz`, and the venue
automatically joins routing, degradation handling, dashboards and alerts.
The Soroban *execution* side of a new venue (if it needs a call adapter in
the merged plan) is open — that part is a normal PR to the contracts in this
repository.

## 4. Code expectations

* TypeScript strict mode; no silent `any` at interface boundaries.
* Errors carry causes: wrap provider errors, never swallow them.
* No secrets in code or fixtures — the repositories are scanned
  (gitleaks) and history rewrites are not an option after publication.
* Behaviour changes ship with their documentation change in the same PR —
  these docs are part of the deliverable, not an afterthought.
* Formatting is enforced by the repo's Prettier config; run it before
  committing.

## 5. Security

Found something security-relevant? **Do not open a public issue.** Follow
`SECURITY.md` in the affected repository for the responsible-disclosure
channel. The audit scope and tagged commits for the 2026 review are listed
in `AUDIT-SCOPE.md`.

## 6. Operational bar

Anything that ships to production is expected to be observable: if your
change adds a failure mode, add the metric or log line that makes it visible,
and if it adds a new external dependency, say how it degrades when that
dependency is down. The [runbook](RUNBOOK.md) shows the standard we hold —
real incidents, documented while fresh.
