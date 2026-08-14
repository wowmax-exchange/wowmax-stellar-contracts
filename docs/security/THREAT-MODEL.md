# STRIDE Threat Model — WOWMAX Stellar Execution Contract

Prepared for the Soroban Security Audit Bank readiness review, following the
[SDF STRIDE template](https://developers.stellar.org/docs/build/security-docs/threat-modeling/STRIDE-template).
Built against the data flow diagram in [DATAFLOW.md](DATAFLOW.md); every
issue below is derived from an interaction (I1–I13) that crosses a trust
boundary (TB1–TB5) defined there.

---

## What are we working on?

`WowmaxAggregator` (crate `wowmax-stellar-router`) is the on-chain execution
layer of the WOWMAX DEX aggregator on Stellar. It is a **plan executor, not a
router**: route selection happens off-chain, and the contract receives an
explicit plan as a call argument, executes exactly that plan across Soroswap,
Aquarius and Phoenix pools in a single atomic invocation, enforces the user's
minimum output on the summed result, and forwards the proceeds.

Deployed on mainnet as
`CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF`,
wasm sha256 `095ee35248f9076fb76d26d7d97e1308c35586df364ff0442b664c5fb3718883`.
Six entry points: `swap` (main, `Vec<Strand>` split + multi-hop),
`swap_merge` (topologically-ordered `Vec<Stage>` DAG with on-chain fan-in),
and four single-venue or two-leg helpers.

Three properties shape the entire analysis:

1. **No persistent state.** No admin address, no configuration, no upgrade
   path, no stored balances. Every value the contract acts on is either a
   call argument or a live balance read. Replacement is by redeployment.
2. **The plan is untrusted input (TB3).** The contract is publicly callable;
   nothing on-chain ties a plan to the WOWMAX pathfinder. It must be safe
   under any plan an attacker can construct.
3. **Venues are untrusted callees (TB4).** Every pool and router address in a
   plan is attacker-choosable, so the contract must survive a venue that
   under-consumes, over-reports, delivers unexpected assets, or is itself
   hostile.

The data flow diagram, entity list, trust boundaries and the numbered
interactions are in [DATAFLOW.md](DATAFLOW.md).

---

## What can go wrong?

Each issue is stated against the interactions it arises from.

| Threat | Issues |
|---|---|
| **S**poofing | **Spoof.1** (I7, I8, TB3) — A caller submits a plan naming a *victim's* address as `user`, attempting to pull the victim's tokens into the contract and route them to a destination of the attacker's choosing.<br><br>**Spoof.2** (I10, I11, TB4) — A plan names an attacker-controlled contract in place of a real venue or a real SAC. The executor then invokes arbitrary code, and reads "balances" from a contract that can return any number it likes. |
| **T**ampering | **Tamper.1** (I11, TB4) — A venue reports a larger output than it actually delivered, so that accounting based on the reported figure would forward tokens the contract does not hold, draining value that belongs to other flows.<br><br>**Tamper.2** (I9, I10, TB4) — A venue under-consumes the input it was authorized to pull (takes part of it, or none), leaving the user's input stranded inside an immutable contract.<br><br>**Tamper.3** (I7, TB3) — A crafted plan desynchronizes venue-level routing data from the hop it claims to execute: a `soroswap_path` or `aqua_pool_tokens` whose endpoints differ from the hop's declared `token_in`/`token_out`.<br><br>**Tamper.4** (I7, TB3) — A crafted plan breaks token continuity — a strand that does not start at `token_in`, a hop that does not consume the previous hop's output, a stage that dead-ends — so the contract spends a token it holds for an unrelated reason. |
| **R**epudiation | **Repudiate.1** (I7–I13) — The executor emits **no contract events of its own**. Reconstructing what a call did — which venues ran, at what split, what each leg produced — depends entirely on SAC transfer events and transaction metadata. A user disputing an execution, or an incident responder reconstructing one, has no first-party audit record from the contract.<br><br>**Repudiate.2** (I7) — Failures surface as `panic!` with string messages rather than typed contract errors, so an integrator cannot programmatically distinguish "deadline passed" from "venue misbehaved" and attribute the failure. |
| **I**nformation Disclosure | **Info.1** (I5, I6, I7, TB2) — The full plan is a public transaction argument. Route structure, split ratios and exact pool selection — the output of the proprietary pathfinder — are disclosed on-ledger with every execution, and are visible to observers before inclusion.<br><br>**Info.2** (I2, I4, TB1) — Between quote and signature the user holds an unsigned envelope whose economic guarantee rests entirely on `amount_out_min` and `deadline`. A user who signs without verifying those two fields discloses nothing, but also retains no protection if the envelope was tampered with in transit. |
| **D**enial of Service | **DoS.1** (I7, I10) — A plan with many strands, hops or fills exhausts the Soroban CPU/memory budget, so the transaction fails at simulation or execution. Self-inflicted for the submitter, but it bounds how complex a route the executor can serve.<br><br>**DoS.2** (I10, I11, TB4) — Any single venue that reverts, or delivers zero output, reverts the entire atomic call. A venue can therefore grief every route that includes it, and a route's success is only as available as its least reliable pool.<br><br>**DoS.3** (I3, I5, TB2) — Quote and submission depend on third-party RPC/Horizon providers; degradation there blocks execution even when the contract and pools are healthy. |
| **E**levation of Privilege | **Elevation.1** (I9, TB4) — The contract authorizes venues to move its own tokens via `authorize_as_current_contract`. An authorization scoped too loosely — wrong recipient, unbounded amount, or a subtree the venue can reuse — would let a venue pull more than the hop intended, or pull again.<br><br>**Elevation.2** (I10, TB4) — A hostile "venue" re-enters the executor during the swap invocation, attempting to have the outer call's balances or authorizations serve the inner one.<br><br>**Elevation.3** (I8, I13, TB3/TB5) — A plan names tokens the contract happens to hold for an unrelated reason (residue, an unsolicited transfer, another user's in-flight balance) and spends them as if they were the caller's capital. |

---

## What are we going to do about it?

| Threat | Remediations |
|---|---|
| **S**poofing | **Spoof.1.R.1** — Every entry point begins with `user.require_auth()`; the input pull is `transfer(user → contract)`, which the Soroban host will not authorize without the user's signature over that exact invocation. Proven by `auth_honest_swap_succeeds` and the three negative auth tests, which run under strict `mock_all_auths()` with no relaxation of `authorize_as_current_contract`.<br>**Spoof.1.R.2** — Proceeds are forwarded to `user` and nowhere else; no plan field names a recipient. `auth_pull_to_rogue_recipient_reverts` covers the attempt.<br>**Spoof.2.R.1** — Naming a hostile contract does not grant it anything: it can only receive what the scoped authorization allows (see Elevation.1.R.1), and it can only be *paid* from the caller's own pulled input. A fake SAC reporting inflated balances harms only the caller who named it, because settlement transfers that same fake token back to that caller. **Residual risk accepted:** a caller who names a hostile token contract loses their own funds; this is inherent to a permissionless executor and is not mitigated on-chain. |
| **T**ampering | **Tamper.1.R.1** — All accounting is by the contract's **own balance delta**, sampled before and after (I12); the venue's returned figure is discarded. `let out = out_after - out_before` gates the payout, so an inflated report cannot exceed what the contract holds. Proven by `inflated_report_is_ignored`, whose mock deliberately over-reports.<br>**Tamper.1.R.2** — In `swap_merge` the payout is the `token_out` delta gained during the call, with the internally tracked figure kept as a lower-bound consistency check (`accounting mismatch` panic). `merge_forwards_all_final_token_out_gained_this_call` covers assets delivered outside a fill's declared destination.<br>**Tamper.2.R.1** — `require_consumed()` asserts that each venue took **exactly** the amount it was handed, turning a stranded balance into a revert. Proven by `venue_that_does_not_pull_input_reverts` and `venue_that_pulls_partially_reverts`.<br>**Tamper.3.R.1** — `require_soroswap_path()` pins the path to exactly `[token_in, token_out]`; `require_aqua_tokens()` pins the pool pair to the hop's declared tokens in either order. Proven by `soroswap_path_wrong_length_reverts` and `soroswap_path_wrong_endpoints_reverts`.<br>**Tamper.4.R.1** — Token continuity is enforced per hop: a strand must start at `token_in`, each hop must consume the previous hop's `token_out`, the last must produce `token_out`, and degenerate hops are rejected. Proven by `plan_must_start_at_token_in`, `plan_must_end_at_token_out`, `self_swap_reverts`.<br>**Tamper.4.R.2** — In `swap_merge`, every tracked token other than `token_out` must end at zero (`plan leaves an unconsumed balance`), so a dead-end stage is a revert rather than a donation to an immutable contract. Proven by `merge_rejects_dead_end_stage` and `merge_zero_weight_fill_reverts`. |
| **R**epudiation | **Repudiate.1.R.1** — **Open finding, proposed for the audit scope.** The executor should emit a per-call event carrying at minimum `user`, `token_in`, `token_out`, `amount_in`, `amount_out_min` and the achieved output, so that execution is attributable from the contract's own record rather than reconstructed from SAC transfers. The adapters in this repository already carry an event module; the executor does not. Not yet implemented — the deployed contract is immutable, so this lands in the next redeployment.<br>**Repudiate.1.R.2** — Interim compensating control: every mainnet execution is recorded with explorer links in `docs/evidence/mainnet-tx.md`, and the operational stack (synthetic prober, dashboards, alerting) is documented in `docs/RUNBOOK.md`.<br>**Repudiate.2.R.1** — **Open finding.** Replace `panic!("…")` with a `#[contracterror]` enum so failures carry stable numeric codes. Same redeployment cycle as Repudiate.1.R.1. Accepted for the current deployment: message strings are stable and asserted by the test suite via `#[should_panic(expected = …)]`. |
| **I**nformation Disclosure | **Info.1.R.1** — Accepted by design. The contract cannot conceal its arguments; on-ledger transparency is a property of the platform. The mitigation is architectural: only the *executed* route is disclosed, never the pathfinder's candidate set, scoring or graph. The algorithm stays off-chain and out of this repository.<br>**Info.1.R.2** — Economic exposure of the disclosed route is bounded by `amount_out_min` and `deadline`: an observer who front-runs cannot push the user below their stated minimum without reverting the call.<br>**Info.2.R.1** — Non-custodial by construction: WOWMAX services never hold keys, sign, or broadcast (see `docs/ARCHITECTURE.md` §4). The returned envelope is unsigned and the wallet displays it before signature.<br>**Info.2.R.2** — `deadline` is enforced **on-chain** rather than delegated to venues, so a stale envelope cannot be replayed later; `expired_deadline_reverts` covers it. A zero deadline is translated to the furthest representable point for venues that compare it against the ledger timestamp, so it cannot silently read as 1970 (`zero_deadline_does_not_reach_the_venue_as_zero`). |
| **D**enial of Service | **DoS.1.R.1** — Budget consumption is measured, not assumed: `budget_heavy_plan` reports CPU consumed by a deliberately heavy plan under `--nocapture`. `swap_merge` exists precisely to keep multi-branch routes inside the budget by issuing one swap per graph edge instead of one per path.<br>**DoS.1.R.2** — The off-chain planner simulates before returning an envelope, so an over-budget plan fails in simulation rather than on-ledger.<br>**DoS.2.R.1** — Accepted: atomicity is the intended trade. A partial fill at a worse price is a worse outcome for the user than a revert. Availability is addressed off-chain — the router ranks alternatives across four venues and the synthetic prober alerts on per-venue degradation (`docs/RUNBOOK.md`).<br>**DoS.3.R.1** — Multi-provider RPC pool with fallback; provider health is probed continuously and alerted on. Out of scope for the contract itself. |
| **E**levation of Privilege | **Elevation.1.R.1** — Each venue authorization is a single `InvokerContractAuthEntry` scoping exactly one `transfer` with the exact token, the exact recipient proven on mainnet for that venue (Soroswap → pool, Aquarius → router, Phoenix → pool), and the exact amount for that hop, with an empty `sub_invocations` subtree. Proven by `auth_pull_more_than_authorized_reverts` and `auth_pull_to_rogue_recipient_reverts`; `auth_replay_transfer_reverts` covers reuse of a consumed authorization.<br>**Elevation.2.R.1** — There is no state to corrupt across a re-entrant call: the contract has no storage, and every decision is re-derived from live balance reads at the moment of use. Combined with `require_consumed()` and the per-call provenance map, a re-entrant path cannot spend value the outer call did not pull or produce. **Flagged for auditor focus:** re-entrancy is the residual risk this design leans hardest on, and deserves explicit adversarial review.<br>**Elevation.3.R.1** — `swap_merge` tracks per-call provenance in an `avail` map seeded solely with the input pulled in this call; only tokens this call pulled or produced may be spent, so balances resting at the address are invisible to a plan. Proven by `merge_cannot_spend_pre_existing_balance` and `merge_sweep_attempt_leaves_balance_untouched`.<br>**Elevation.3.R.2** — `out_before` and `mid_before` are sampled **before** the input is pulled, so every delta covers only value that arrived during the call; pre-existing dust cannot be swept into a payout. `no_funds_retained_after_swap` asserts the contract holds nothing afterwards.<br>**Elevation.3.R.3** — **Residual risk documented and accepted:** the contract is immutable with no admin, so tokens sent to its address outside a swap, or side-transferred by a venue in an asset the route never declared, are unrecoverable by anyone. This is recorded in `public/mainnet.contracts.json` and is the deliberate price of having no privileged role. |

---

## Did we do a good job?

**Has the data flow diagram been referenced since it was created?**
Yes. Every issue above is anchored to numbered interactions and trust
boundaries from it, and the boundary that carries the most issues (TB4,
executor ↔ venue) is the one the diagram highlights.

**Did the STRIDE model uncover any new design issues or concerns that had not
been previously addressed?**
Yes, two, both under Repudiation. The executor emits no first-party events
and reports failures as panic strings rather than typed error codes. Neither
had been treated as a security concern before — they had been filed as
ergonomics. Framed as repudiation, they become an attribution gap: a disputed
execution cannot be reconstructed from the contract's own record. Both are
recorded as open findings for the next redeployment, since the current
contract is immutable.

The exercise also sharpened an existing property into an explicit statement:
the pathfinder is **not** a security dependency. That had been implicit in
the design; stating it as a trust boundary is what makes the proprietary
off-chain component legitimately out of scope for an audit of the contract.

**Did the treatments adequately address the issues identified?**
For Spoofing, Tampering and Elevation of Privilege the treatments are
implemented in the deployed contract and each is backed by a named test —
notably the hostile-venue cases (over-reporting, under-consuming,
side-transferring) and the merge provenance cases, which came from real
review findings that superseded three earlier deployments, as recorded in
`public/mainnet.contracts.json`. Information Disclosure and Denial of Service
carry accepted risks stated explicitly rather than papered over. Repudiation
is the one category where treatment is planned, not delivered.

**Have additional issues been found after the threat model?**
The model is new. Its immediate predecessors were adversarial review rounds
that produced four superseded deployments, each documented with the specific
weakness it fixed — the most serious being a `swap_merge` that treated the
contract's absolute balance as spendable capital (never exploited; balances
were verified zero throughout).

**Any additional thoughts or insights on the process?**
The most useful output was the forced separation between "what the contract
guarantees" and "what the off-chain stack guarantees". Several protections we
had been describing as system properties turned out, on inspection, to live
entirely off-chain — which is exactly what an attacker calling the contract
directly would bypass. The model will be revisited at the next redeployment,
when the event emission and typed errors land.
