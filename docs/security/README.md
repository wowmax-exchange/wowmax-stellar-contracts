# Audit Readiness — WOWMAX Stellar Execution Contract

Submission index for the [Soroban Security Audit Bank](https://stellar.gitbook.io/scf-handbook/supporting-programs/audit-bank/official-rules).
Each row of the readiness checklist maps to the artifact that satisfies it.

## Audit target

| | |
|---|---|
| **Contract** | `WowmaxAggregator`, crate `wowmax-stellar-router` |
| **Mainnet address** | [`CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF`](https://stellar.expert/explorer/public/contract/CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF) |
| **Wasm sha256** | `095ee35248f9076fb76d26d7d97e1308c35586df364ff0442b664c5fb3718883` |
| **Entry points** | `swap`, `swap_merge`, `swap_soroswap`, `swap_aqua`, `swap_phoenix`, `swap_aqua_then_soroswap` |
| **Privileged roles** | none — no admin function, no upgrade path, no contract storage |
| **Category** | Financial protocol: routes user funds through third-party AMMs within a single atomic invocation |
| **Funding** | Stellar Community Fund Build Award #43 |

Out of scope, with reasoning, in [DATAFLOW.md](DATAFLOW.md): the adapters
(not wired into the executor, not deployed), the deployer helper (not on
mainnet), the off-chain pathfinder (proprietary, and by design not a security
dependency — the contract treats its output as untrusted input), and the
classic SDEX path (executed at protocol level, not through this contract).

## Checklist

| # | Requirement | Evidence |
|---|---|---|
| 1 | **Funding** — SCF-funded and eligible | Build Award #43; contract live on mainnet |
| 2 | **Repo hygiene** — organised, understandable | Workspace of 8 crates, the audit target isolated in `contracts/router` (1,049 lines, single file). Third-party binaries inventoried in [THIRD-PARTY-WASM.md](THIRD-PARTY-WASM.md) |
| 3 | **Integration tests present and executed** | [TEST-EVIDENCE.md](TEST-EVIDENCE.md) — 127 passed, 0 failed, with toolchain and command. The executor is exercised both against a deliberately hostile mock and against real Soroswap factory/pair/router contracts. Re-run on every push by [`ci.yml`](../../.github/workflows/ci.yml) |
| 4 | **Threat model** — completed, assessed against the dataflow | [THREAT-MODEL.md](THREAT-MODEL.md) — STRIDE per the SDF template, 15 issues with unique IDs, each derived from a numbered interaction and each remediation tied to the test that proves it |
| 5 | **Dataflow diagram** — trust boundaries and data entities identified | [DATAFLOW.md](DATAFLOW.md) — 5 trust boundaries, 13 interactions, entity and data inventory |
| 6 | **Tooling scan** (bonus per the checklist, **required** per Official Rules) | *in progress — Scout static analysis* |
| 7 | **Remediation plan** for scan findings | *follows the scan* |

## Reproducibility

The wasm rebuilt from the committed sources is byte-identical to the hash
recorded for the deployed contract — verified with `stellar 26.0.0` /
`cargo 1.95.0` / `rustc 1.95.0`, target `wasm32v1-none`. An auditor can
therefore review sources knowing they correspond to the live bytecode. The
check is part of [TEST-EVIDENCE.md](TEST-EVIDENCE.md) and is regenerated from
the raw build and test output rather than transcribed.

## Known open findings

Declared up front rather than left for the audit to discover. Both are
attribution gaps under Repudiation in the threat model, and both require a
redeployment because the contract is immutable:

1. The executor emits **no contract events of its own**; execution is
   reconstructed from SAC transfer events and transaction metadata.
2. Failures are `panic!` strings rather than a `#[contracterror]` enum, so an
   integrator cannot distinguish failure causes programmatically.

Accepted risks — an immutable contract cannot recover tokens sent to it
outside a swap; a caller who names a hostile token contract loses their own
funds; atomicity means one failing venue reverts the whole route — are stated
with their reasoning in [THREAT-MODEL.md](THREAT-MODEL.md).

## Prior review history

Four superseded deployments are recorded in
[`public/mainnet.contracts.json`](../../public/mainnet.contracts.json), each
with the specific weakness that motivated its replacement. The most serious:
an earlier `swap_merge` treated the contract's absolute token balance as
spendable capital rather than tracking per-call provenance. It was never
exploited — balances were verified zero throughout — and the current build
carries both the provenance map and the regression tests that pin it.
