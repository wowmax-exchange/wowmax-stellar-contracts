# WOWMAX Stellar Router — Deliverable 4 Report
## Extended DEX Integration: Phoenix + Aquarius

*Generated:* 2026-06-28T04:07:28.524Z
*Network:* Stellar mainnet  ·  *RPC:* https://rpc.ankr.com/…
*Token universe:* XLM, USDC, AQUA, EURC, yXLM, VELO, XRP

All figures below are computed from live mainnet liquidity at generation
time. Classic (SDEX path payments) and Soroban (contract calls) are never
mixed in one route; each route is `max(classic, soroban)`. Phoenix and
Aquarius are Soroban venues, so they enrich the Soroban side only.

## Deliverable criteria

| Criterion | Result |
|---|---|
| 4 DEXes active in the full graph | ✅ PASS |
| >=10 benchmark pairs | ✅ PASS |
| >=1 pair improved vs 2-DEX baseline | ✅ PASS |
| >=1 multi-venue Soroban route | ✅ PASS |
| failover: every pair still routes under each single-venue outage | ✅ PASS |
| live forced-outage build survived + quoted | ✅ PASS |
| **Overall** | ✅ **All criteria met** |

## 1. Optimal routing across 4 Stellar DEXes

Active venues in the full graph: **sdex, soroswap, phoenix, aqua** (4/4).

Per-venue directed edges discovered over the token universe (`ok` = pair
produced edges, `no-pool` = venue reachable but no pool for that pair,
`error` = read failed):

| Venue | ok | no-pool | error | last error |
|---|---:|---:|---:|---|
| sdex | 42 | 0 | 0 | — |
| soroswap | 16 | 13 | 0 | — |
| phoenix | 4 | 19 | 0 | — |
| aqua | 13 | 11 | 0 | — |

Graph size — baseline (SDEX+Soroswap): 42 classic / 16 soroban edges, built in 2377ms. Full (4-DEX): 42 classic / 46 soroban edges, built in 14383ms.

> Soroban resource-limit pruning is enforced at quote time by the server's
> weighted-hop budget (concentrated Aquarius hop = 2 units, every other hop
> = 1, cap `W_MAX = 12` ≈ the ~90M-instruction working ceiling). An optimal
> route exceeding the budget is simplified (drop lowest-flow edges,
> re-optimise) until it is on-chain executable, and the `/swap` path
> re-simulates and skips any candidate that still trips `ExceededLimit`.

## 2–3. Benchmark: 4-DEX vs 2-DEX baseline

`baseline = SDEX + Soroswap`  ·  `full = SDEX + Soroswap + Phoenix + Aquarius`

| Pair (amount in) | Base out | Full out | Δ bps | Full route | Venues |
|---|---:|---:|---:|---|---|
| XLM->USDC (100) | 17.3253588 | 17.3253588 | 0.0 | single | sdex |
| XLM->USDC (1000) | 173.2521588 | 173.2521588 | 0.0 | single | sdex |
| XLM->USDC (10000) | 1732.3061581 | 1732.3061581 | 0.0 | single | sdex |
| XLM->USDC (100000) | 17262.5716881 | 17262.5716881 | 0.0 | multi-hop | sdex |
| USDC->XLM (5000) | 28790.4786706 | 28790.4786706 | 0.0 | multi-hop | sdex |
| USDC->EURC (500) | 439.8629961 | 440.1893974 | 7.4 | multi-hop * | soroswap+aqua |
| USDC->EURC (5000) | 4387.9277139 | 4387.9277139 | 0.0 | single | sdex |
| EURC->USDC (500) | 563.1702605 | 563.4420770 | 4.8 | split * | aqua+soroswap |
| XLM->EURC (1000) | 153.0752628 | 153.0782103 | 0.2 | split * | phoenix+soroswap |
| XLM->EURC (10000) | 1520.8124139 | 1522.5604352 | 11.5 | multi-hop * | phoenix+soroswap+aqua |
| USDC->AQUA (1000) | 2900582.4962269 | 2901472.2756351 | 3.1 | split * | aqua |
| XLM->AQUA (10000) | 5008647.7226151 | 5008647.7226151 | 0.0 | single | sdex |
| AQUA->EURC (1000) | 0.3016487 | 0.3038410 | 72.7 | multi-hop * | aqua+soroswap+phoenix |
| AQUA->USDC (50000) | 17.0577970 | 17.1266263 | 40.4 | single * | aqua |
| XLM->VELO (10000) | 538971.2064065 | 538971.2064065 | 0.0 | single | sdex |
| VELO->XRP (200000) | 604.7830367 | 604.7830367 | 0.0 | multi-hop | sdex |
| VELO->EURC (600000) | 1667.7433727 | 1667.7433727 | 0.0 | multi-hop | sdex |
| XRP->USDC (500) | 523.2992898 | 523.2992898 | 0.0 | multi-hop | sdex |

`*` = the winning full route uses Phoenix and/or Aquarius.

- Pairs benchmarked: **18**
- Pairs improved by the 4-DEX graph: **7**
- Pairs whose winning route uses a new venue (Phoenix/Aquarius): **7**
- Max improvement: **72.7 bps**  ·  avg (improved): **20.0 bps**
- Venue participation across winning routes: `{"sdex":11,"soroswap":5,"aqua":6,"phoenix":3}`

> Δ bps measures full-graph output vs the 2-DEX baseline for the SAME pair
> and size. A near-zero Δ on a pair means the baseline venues already held
> the best price for that size; it is not a regression — the full graph
> never returns less than the baseline (it is a superset of liquidity).

## 4. Multi-hop routing across multiple Soroban DEXes

Routes whose winning Soroban path spans **≥2 distinct Soroban DEXes**: **5**. Routes executing as a sequential multi-hop (≥2 stages) through Soroban: **3**.

| Pair | Out | Soroban venues | Type | Stages | Strand |
|---|---:|---|---|---:|---|
| USDC->EURC (500) | 440.1893974 | soroswap+aqua | multi-hop | 2 | `USDC:GA5Z..K4KZVN =[soroswap->XLM@3% | aqua->EURC:GDHU..ITNPP2@46% | soroswap->EURC:GDHU..ITNPP2@51%]  >>  XLM =[soroswap->EURC:GDHU..ITNPP2@100%]` |
| EURC->USDC (500) | 563.4420770 | aqua+soroswap | split | 1 | `EURC:GDHU..ITNPP2 =[aqua->USDC:GA5Z..K4KZVN@54% | soroswap->USDC:GA5Z..K4KZVN@46%]` |
| XLM->EURC (1000) | 153.0782103 | phoenix+soroswap | split | 1 | `XLM =[phoenix->EURC:GDHU..ITNPP2@2% | soroswap->EURC:GDHU..ITNPP2@98%]` |
| XLM->EURC (10000) | 1522.5604352 | phoenix+soroswap+aqua | multi-hop | 2 | `XLM =[phoenix->USDC:GA5Z..K4KZVN@25% | soroswap->USDC:GA5Z..K4KZVN@6% | phoenix->EURC:GDHU..ITNPP2@1% | soroswap->EURC:GDHU..ITNPP2@68%]  >>  USDC:GA5Z..K4KZVN =[aqua->EURC:GDHU..ITNPP2@45% | soroswap->EURC:GDHU..ITNPP2@55%]` |
| AQUA->EURC (1000) | 0.3038410 | aqua+soroswap+phoenix | multi-hop | 4 | `AQUA:GBNZ..67AQUA =[aqua->XRP:GBXR..PRDTD5@78% | aqua->USDC:GA5Z..K4KZVN@12% | soroswap->USDC:GA5Z..K4KZVN@10%]  >>  USDC:GA5Z..K4KZVN =[soroswap->XLM@37% | aqua->EURC:GDHU..ITNPP2@63%]  >>  XRP:GBXR..PRDTD5 =[aqua->XLM@100%]  >>  XLM =[phoenix->EURC:GDHU..ITNPP2@100%]` |

> Strand notation: each stage is `TOKEN =[venue->dst@share% | …]`, stages
> joined by `>>`. A split shows multiple legs in one stage; a multi-hop
> shows multiple stages. The aggregator splits a single swap across pools
> and venues and executes it atomically in one transaction.

## 5. Failover

Each scenario removes a venue's liquidity from the full graph, re-optimises
on the surviving edges, and checks that every previously-routable pair still
produces a valid quote. `Δ bps` is the degraded output vs the full-graph
output (negative = the lost venue was contributing; the route is still
served by the rest). `single` scenarios (one venue down) define the
pass/fail; `extreme` scenarios (a whole execution mode down) are
informational — a pair with only-Soroban liquidity legitimately cannot be
served when all Soroban venues are down.

| Scenario | Type | Pair | Full out | Degraded out | Δ bps | Still routes | Degraded venues |
|---|:---:|---|---:|---:|---:|:---:|---|
| drop:soroswap | single | XLM->USDC (10000) | 1732.3061581 | 1732.3061581 | 0.0 | ✅ | sdex |
| drop:soroswap | single | USDC->AQUA (1000) | 2901472.2756351 | 2901472.2756351 | 0.0 | ✅ | aqua |
| drop:soroswap | single | AQUA->USDC (50000) | 17.1266263 | 17.1266263 | 0.0 | ✅ | aqua |
| drop:soroswap | single | VELO->XRP (200000) | 604.7830367 | 604.7830367 | 0.0 | ✅ | sdex |
| drop:soroswap | single | XLM->EURC (10000) | 1522.5604352 | 1520.7079296 | -12.2 | ✅ | sdex |
| drop:phoenix | single | XLM->USDC (10000) | 1732.3061581 | 1732.3061581 | 0.0 | ✅ | sdex |
| drop:phoenix | single | USDC->AQUA (1000) | 2901472.2756351 | 2901472.2756351 | 0.0 | ✅ | aqua |
| drop:phoenix | single | AQUA->USDC (50000) | 17.1266263 | 17.1266263 | 0.0 | ✅ | aqua |
| drop:phoenix | single | VELO->XRP (200000) | 604.7830367 | 604.7830367 | 0.0 | ✅ | sdex |
| drop:phoenix | single | XLM->EURC (10000) | 1522.5604352 | 1520.8852424 | -11.0 | ✅ | soroswap+aqua |
| drop:aqua | single | XLM->USDC (10000) | 1732.3061581 | 1732.3061581 | 0.0 | ✅ | sdex |
| drop:aqua | single | USDC->AQUA (1000) | 2901472.2756351 | 2900582.4962269 | -3.1 | ✅ | sdex |
| drop:aqua | single | AQUA->USDC (50000) | 17.1266263 | 17.0577970 | -40.2 | ✅ | sdex |
| drop:aqua | single | VELO->XRP (200000) | 604.7830367 | 604.7830367 | 0.0 | ✅ | sdex |
| drop:aqua | single | XLM->EURC (10000) | 1522.5604352 | 1522.1810359 | -2.5 | ✅ | phoenix+soroswap |
| classic-only (all Soroban down) | extreme | XLM->USDC (10000) | 1732.3061581 | 1732.3061581 | 0.0 | ✅ | sdex |
| classic-only (all Soroban down) | extreme | USDC->AQUA (1000) | 2901472.2756351 | 2900582.4962269 | -3.1 | ✅ | sdex |
| classic-only (all Soroban down) | extreme | AQUA->USDC (50000) | 17.1266263 | 17.0577970 | -40.2 | ✅ | sdex |
| classic-only (all Soroban down) | extreme | VELO->XRP (200000) | 604.7830367 | 604.7830367 | 0.0 | ✅ | sdex |
| classic-only (all Soroban down) | extreme | XLM->EURC (10000) | 1522.5604352 | 1520.7079296 | -12.2 | ✅ | sdex |
| soroban-only (SDEX down) | extreme | XLM->USDC (10000) | 1732.3061581 | 1722.6878808 | -55.5 | ✅ | aqua+soroswap+phoenix |
| soroban-only (SDEX down) | extreme | USDC->AQUA (1000) | 2901472.2756351 | 2901472.2756351 | 0.0 | ✅ | aqua |
| soroban-only (SDEX down) | extreme | AQUA->USDC (50000) | 17.1266263 | 17.1266263 | 0.0 | ✅ | aqua |
| soroban-only (SDEX down) | extreme | VELO->XRP (200000) | 604.7830367 | 267.5155379 | -5576.7 | ✅ | aqua+soroswap |
| soroban-only (SDEX down) | extreme | XLM->EURC (10000) | 1522.5604352 | 1522.5604352 | 0.0 | ✅ | phoenix+soroswap+aqua |

Every pair still routes under every single-venue outage: **yes** (15/15 single-outage checks passed).

### Live forced-outage (runtime isolation)

A graph built with the Aquarius loader forced to throw on every read — the
real runtime failover path, not edge arithmetic. The loader's per-pool
`try/catch` converts the outage into `error` diagnostics and the venue is
simply absent; the build succeeds and routing continues on the other DEXes.

- Build completed in **1662ms** (no throw escaped to the caller).
- Aquarius diagnostics: ok=**0**, error=**21** (expected: ok=0, error>0 — venue cleanly down).
- Surviving graph: 42 classic / 20 soroban edges.
- Sample quote during outage — XLM->USDC (10000): **1731.9983662** via `sdex`.

> Loader-level isolation is also covered by the offline unit suite
> (`tests/builder.test.ts`: a throwing loader is captured as an `error`
> diagnostic and never sinks the graph).

---

*Report produced by `src/cli/d4-report.ts`. Reproduce with* `npx tsx src/cli/d4-report.ts > D4_REPORT.md`*.*
