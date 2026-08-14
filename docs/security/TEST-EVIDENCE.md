# Test Evidence

Execution record for the Soroban Security Audit Bank readiness review (checklist item: *integration testing code present and executed*). Generated from the raw `cargo test --workspace` output.

* **Run at:** 2026-08-14 13:32 UTC
* **Commit:** `59549912e2672cf8d6ac556a41234122b4c46259`  ⚠️ working tree not clean at generation time
* **Command:** `cd contracts && stellar contract build && cargo test --workspace`
* **Toolchain:** stellar 26.0.0 (60f7458e7ecffddf2f2d91dc6d0d2db4fab03ecc) · cargo 1.95.0 (f2d3ce0bd 2026-03-21) · rustc 1.95.0 (59807616e 2026-04-14)
* **Wasm target:** `wasm32v1-none`
* **Third-party wasm fixtures in tree:** 19 (inventory: [THIRD-PARTY-WASM.md](THIRD-PARTY-WASM.md))

## Result

**127 passed · 0 failed · 0 ignored**

| Crate | Passed | Failed | Ignored | In audit scope |
|---|---:|---:|---:|---|
| `wowmax_aqua_adapter` | 23 | 0 | 0 | no |
| `wowmax_comet_adapter` | 21 | 0 | 0 | no |
| `wowmax_phoenix_adapter` | 25 | 0 | 0 | no |
| `wowmax_soroswap_adapter` | 26 | 0 | 0 | no |
| `wowmax_stellar_router` | 32 | 0 | 0 | **yes — the deployed executor** |

Suites reporting no tests (interface crate, test helpers, doc-tests): test_utils, wowmax_adapter_interface, wowmax_stellar_deployer.

## Reproducibility of the deployed contract

The executor wasm built from this commit was compared against the hash recorded for the contract live on mainnet:

* Address: `CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF`
* Recorded on-chain deployment: `095ee35248f9076fb76d26d7d97e1308c35586df364ff0442b664c5fb3718883`
* Rebuilt from source: `095ee35248f9076fb76d26d7d97e1308c35586df364ff0442b664c5fb3718883`
* **Result: byte-identical — the repository reproduces the deployed contract**

## What the executor suite covers

The suite for the in-scope contract is organised around the threats in [THREAT-MODEL.md](THREAT-MODEL.md); the mapping from each remediation to the test that proves it is given there. Two groups are worth calling out:

* **Hostile-venue behaviour** — a mock venue that can over-report its output, refuse to consume the input it was handed, consume only part of it, or side-transfer an asset the route never declared.

* **Authorization** — a group running under strict `mock_all_auths()`, which does *not* relax `authorize_as_current_contract`, covering an honest swap plus three attempts to abuse the pre-authorization: a rogue recipient, an amount above the authorized cap, and replay of a consumed entry.

## Reproducing this

```bash
cd contracts
stellar contract build
cargo test --workspace 2>&1 | tee /tmp/ctest.log
```

Continuous verification: [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) runs the same two commands on every push and pull request.
