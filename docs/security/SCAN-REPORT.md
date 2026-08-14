# Static Analysis Report

Self-service tooling scan for the Soroban Security Audit Bank submission,
covering the audit target `wowmax-stellar-router`.

* **Tool:** Scout (CoinFabrik) `cargo-scout-audit` v0.3.16 — from the
  [Stellar security tools list](https://developers.stellar.org/docs/tools/developer-tools/security-tools)
* **Scanned:** `contracts/router` — the deployed executor
* **Detectors available:** 36 · **Findings:** 43 · **Real vulnerabilities: 0**
* **Toolchain:** detectors on `nightly-2025-08-07`; project on `stellar 26.0.0` / `cargo 1.95.0` / `rustc 1.95.0`

## Results by detector

| Detector | Severity | Hits | Disposition |
|---|---|---:|---|
| `integer_overflow_or_underflow` | Critical | 22 | False positive — see below |
| `unsafe_unwrap` | Medium | 9 | False positive — bounded indices |
| `avoid_vec_map_input` | Medium | 6 | Mitigated — validation is in the body |
| `unsafe_map_get` | Medium | 5 | False positive — `unwrap_or(0)` |
| `soroban_version` | Enhancement | 1 | Accepted — version pinned deliberately |

### `integer_overflow_or_underflow` — 22 hits, no action

The release profile sets `overflow-checks = true` with `panic = "abort"`, so
arithmetic overflow is not a silent wraparound but a panic, which reverts the
whole transaction. Every arithmetic site therefore fails closed. The detector
warns about the silent-wraparound class, which this build cannot exhibit.

By shape, the 22 sites are:

* **8 balance deltas** — `out_after - out_before`, `mid_after - mid_before`,
  `hop_after - hop_before`, `dst_after - dst_before`, and `before - after` in
  `require_consumed`. Differences of two non-negative token balances, both
  bounded by `i128` supply.
* **2 proportional splits** — `(amount_in * parts) / total_parts` and
  `(bal * parts) / total_parts`. Multiplication precedes division to preserve
  precision; the product is bounded by the pulled input times the summed
  parts, and overflow would revert.
* **4 accumulators** — `total_parts +=`, `allocated +=`, bounded by the plan.
* **2 loop bounds** — `s == n - 1`, `fi == fcount - 1`, guarded by
  `n == 0` / `fcount == 0` panics before the loop.
* **6 provenance arithmetic** in `swap_merge` — `avail` map updates, each
  operating on values the same call produced.

Division by zero is impossible: both `swap` and `swap_merge` panic on
`total_parts <= 0` before any division.

### `unsafe_unwrap` — 9 hits, no action

All nine are `Vec::get(i).unwrap()` where `i` is the induction variable of a
`while i < n` loop and `n` is that collection's own `len()`. The two
non-loop cases, `tokens.get(0)` and `tokens.get(1)` in `require_aqua_tokens`,
are preceded by an explicit `tokens.len() != 2` panic; the equivalent
`path.get(0)/get(1)` in `require_soroswap_path` is preceded by its own length
check, pinned by the test `soroswap_path_wrong_length_reverts`.

### `avoid_vec_map_input` — 6 hits, already mitigated

The detector flags `Vec` parameters on entry points without validated
contents. This is the same concern the threat model records as **TB3** — the
plan is untrusted input — and it is the single assumption the contract is
built around. Validation is present but lives in the body rather than the
signature, so a signature-level detector cannot see it:

| Guard | Enforces | Test |
|---|---|---|
| `require_soroswap_path` | path is exactly `[token_in, token_out]` | `soroswap_path_wrong_length_reverts`, `soroswap_path_wrong_endpoints_reverts` |
| `require_aqua_tokens` | pool pair matches the hop, either order | covered in the auth and plan-shape group |
| token continuity | strand starts at `token_in`, each hop consumes the previous output, last produces `token_out` | `plan_must_start_at_token_in`, `plan_must_end_at_token_out` |
| `require_consumed` | venue took exactly what it was handed | `venue_that_does_not_pull_input_reverts`, `venue_that_pulls_partially_reverts` |
| `avail` provenance map | only tokens this call pulled or produced may be spent | `merge_cannot_spend_pre_existing_balance`, `merge_sweep_attempt_leaves_balance_untouched` |
| empty-collection panics | no empty plan, strand, or stage | `empty_plan_reverts` |

Converting these to contract-defined types, as the detector suggests, would
not add a guarantee: the executor is permissionless, so any type a caller can
construct is still attacker-chosen. The check must be semantic, which is what
the body does.

### `unsafe_map_get` — 5 hits, false positive

Every flagged call is `avail.get(...).unwrap_or(0)`, not a bare `get`. A
missing key yields zero, which is the correct semantics: a token this call has
not touched has no current-call balance. The detector matches on `.get()`
without following the `unwrap_or`.

### `soroban_version` — 1 hit, accepted

The project pins `soroban-sdk 25.3.1` deliberately. The wasm rebuilt from
source at this pin is byte-identical to the contract deployed on mainnet
(`095ee35248f9076fb76d26d7d97e1308c35586df364ff0442b664c5fb3718883`), which is
the strongest correspondence evidence available to an auditor. Changing the
SDK before the audit would break it. Revisited at the next redeployment.

## Remediation plan

No finding requires a code change. Per Official Rules, critical, high and
medium findings must be resolved before the audit begins; each is resolved
here by demonstrating the guarantee already exists — in the build profile, in
a preceding bounds check, or in a named test — rather than by modification.

Two open items stand, both from the threat model rather than from this scan,
and both requiring redeployment because the contract is immutable: the
executor emits no contract events of its own, and failures are panic strings
rather than a `#[contracterror]` enum. See
[THREAT-MODEL.md](THREAT-MODEL.md), Repudiate.1 and Repudiate.2.

## Reproducing this scan

Scout v0.3.16 does not run against `soroban-sdk 25.3.x` out of the box, for
two independent reasons, each with a documented workaround:

1. Its detectors are pinned to `nightly-2025-08-07`, on which
   `soroban-sdk-macros 25.3.1` fails to compile — it calls
   `floor_char_boundary`, stabilized later. Injecting the feature attribute
   into **host** artifacts resolves it; `build.rustflags` does not reach
   proc-macros, so `host.rustflags` under `-Zhost-config` is required.
2. It hardcodes `--target=wasm32-unknown-unknown`
   (`src/cli_args/mod.rs:305-315`), a target `soroban-sdk` rejects on
   rustc 1.82+. It skips its default when the passed arguments already
   contain `--target=`, so the supported `wasm32v1-none` can be substituted.

```bash
rustup target add wasm32v1-none --toolchain nightly-2025-08-07
cd contracts
cargo scout-audit -m router/Cargo.toml -o json --output-path scout.json -- \
  -Zhost-config -Ztarget-applies-to-host \
  --config "target-applies-to-host=false" \
  --config "host.rustflags=[\"-Zcrate-attr=feature(round_char_boundary)\"]" \
  --target=wasm32v1-none
```

**Verify the scan actually ran.** When a dependency's build script panics,
Scout still prints `Analyzed` with zero counts in every column and writes a
report with an empty `findings` array — indistinguishable from a clean result.
Our first three attempts produced exactly that. Before trusting any Scout
output, confirm the log contains no `build failed`, no `error`, and no
`Compilation errors` status.
