# Security Policy

## Reporting a vulnerability

Report suspected vulnerabilities privately to **max@wowmax.exchange**.

Please do not open a public issue, and do not disclose the finding publicly
until we have had a chance to respond. If you believe user funds are at
immediate risk, say so in the subject line.

A useful report includes:

* the affected contract address or repository path, and the commit if known;
* what an attacker can achieve, and under what preconditions;
* a reproduction — a transaction hash, a failing test, or a plan structure
  that triggers the behaviour.

We will acknowledge your report, keep you informed while we investigate, and
tell you what we decide to do. If you would like credit for the finding, say
so and we will name you; if you prefer to stay anonymous, we will respect
that.

## Scope

**In scope — the deployed execution contract:**

| | |
|---|---|
| Contract | `WowmaxAggregator`, crate `wowmax-stellar-router` |
| Mainnet address | `CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF` |
| Wasm sha256 | `095ee35248f9076fb76d26d7d97e1308c35586df364ff0442b664c5fb3718883` |

We are particularly interested in anything that lets a caller move value that
is not theirs, lets a venue take more than the plan authorized, leaves funds
stranded in the contract, or bypasses the `amount_out_min` and `deadline`
guarantees.

**Out of scope:**

* The adapters (`contracts/adapters/*`) and the deployer helper — not wired
  into the executor and not deployed on mainnet.
* Third-party venue contracts (Soroswap, Aquarius, Phoenix) and the wasm
  fixtures in this repository, which are test artifacts of those projects.
  Report those to their maintainers.
* Findings that require a compromised user wallet or a user voluntarily
  naming a hostile token contract in their own plan.
* Already-documented accepted risks — see
  [docs/security/THREAT-MODEL.md](docs/security/THREAT-MODEL.md). Notably: the
  contract is immutable with no admin, so tokens sent to its address outside a
  swap cannot be recovered by anyone.

## What this contract is

An immutable plan executor with no admin function, no upgrade path and no
contract storage. It treats the swap plan as untrusted input and the venues it
calls as untrusted callees. The threat model and the data flow diagram behind
those statements are in [docs/security/](docs/security/).

## Known open items

Recorded here so nobody spends time rediscovering them:

* The executor emits no contract events of its own; execution is reconstructed
  from token transfer events and transaction metadata.
* Failures surface as panic messages rather than typed contract errors.

Both are attribution gaps rather than fund-safety issues, and both are queued
for the next redeployment — the deployed contract cannot be upgraded in place.
