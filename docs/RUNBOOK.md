# WOWMAX Stellar Stack — Operations Runbook

SCF Build Award #43 — Deliverable 8 (Monitoring & Production Operations).
Last updated: 2026-08-02.

This runbook covers the production operation of the WOWMAX Stellar aggregation
stack: the DEX routing service, the bridge aggregator, the public API gateway
path, and the monitoring layer itself. Each incident scenario lists symptoms
(which alert fires, what the dashboards show), diagnosis steps, mitigation,
and — where we have already lived through the incident — a reference to the
real case.

---

## 1. System map

| Component            | Where it runs                  | Port  | Health check                     |
|----------------------|--------------------------------|-------|----------------------------------|
| Stellar router v2    | routing host, systemd `wowmax-stellar-router` | 8083 (localhost) | `GET /healthz` |
| bridge-aggregator    | routing host, node process     | 8085 (localhost) | `GET /health`                    |
| Caddy (public edge of the routing host) | routing host, systemd `caddy` | 443 | serves `stellar-router.wowmax.exchange` |
| api-gateway          | k8s cluster (public)           | 443   | `GET /docs`, `GET /chains/100000148/quote?...` |
| VictoriaMetrics      | routing host, docker `vmstellar` | 8428 (localhost, basic auth) | `GET /health` |
| stellar-prober       | routing host, systemd `stellar-prober` | 9105 (localhost) | `GET /metrics` |
| Grafana              | internal monitoring stack      | 443   | dashboards `Stellar Router & DEX Venues`, `Stellar Bridges` |

Public exposure of the routing host: Caddy terminates TLS for
`stellar-router.wowmax.exchange` and path-splits it — `/bridge/*` goes to the
bridge-aggregator (:8085), everything else to the router v2 (:8083). This is
also how the k8s api-gateway reaches both services (its `BRIDGE_AGGREGATOR_URL`
and `STELLAR_ROUTER_URL` point at this hostname), so a Caddy outage breaks the
public gateway path while both local services stay healthy — see Scenario 7.

Data flow: `stellar-prober` probes the router (`/healthz`, `/quote`,
`/rpcpool-stats`), the bridge aggregator (`/health`, `POST /bridge/quote`) and
the public gateway path every 90/180/120 seconds, exposing Prometheus metrics
on `:9105`. VictoriaMetrics scrapes the prober every 30 s. Grafana reads
VictoriaMetrics through the `VM Stellar` datasource; alert rules live in the
`Stellar` folder, rule group `stellar-health` (60 s evaluation), and route via
the `team=stellar` notification policy to Telegram.

DEX venues covered: SDEX (classic order book), Soroswap, Phoenix, Aquarius.
Bridges covered on Stellar routes: **Near Intents** (reached through the
Aurora Intents gateway), **Squid** — which is also how Axelar's ITS Hub
serves Stellar — and **Allbridge** (currently gated off, see Scenario 6),
plus composite routes (bridge + WOWMAX Stellar leg). A direct Axelar adapter
also exists but is scoped to the EVM chains where Axelar has gateway assets;
on Stellar it is skipped (`CC_AXELAR_SKIP_CHAINS`, default `stellar`) and its
unsupported-route rows are suppressed (`CC_SILENT_UNSUPPORTED`, default
`axelar`) so the ranking shows options rather than noise.

## 2. Alert catalogue

| Alert (uid)                              | Condition                                  | For  | Severity |
|------------------------------------------|--------------------------------------------|------|----------|
| Stellar: router down (r1)                | `stellar_router_up < 1`                    | 3m   | critical |
| Stellar: venue not serving (r2)          | `stellar_venue_serving < 1` per venue      | 15m  | warning  |
| Stellar: DEX probe failing (r3)          | `stellar_dex_probe_ok < 1` per pair        | 10m  | warning  |
| Stellar: bridge-aggregator down (r4)     | `stellar_bridge_service_up < 1`            | 3m   | critical |
| Stellar: no direct bridge for pair (r5)  | `sum by(pair)(stellar_bridge_quote_ok) < 1`| 15m  | warning  |
| Stellar: gateway public e2e down (r6)    | `stellar_gateway_probe_ok < 1`             | 5m   | critical |
| Stellar: router quote p95 high (r7)      | p95(router_quote by pair) > 20s over 10m   | 15m  | warning  |
| Stellar: prober down (r8)                | `up{job="stellar_prober"} < 1`             | 5m   | critical |

Design note: r1–r7 use `noDataState=OK` so that a dead prober produces exactly
one alert — r8, the sentinel — instead of a storm of `DatasourceNoData`.

## 3. Incident scenarios

### Scenario 1 — Router down (r1, critical)

**Symptoms.** r1 fires; `Router` stat panel red; every DEX probe fails
simultaneously; the public gateway probe (r6) usually follows within minutes.

**Diagnose.**

```bash
systemctl status wowmax-stellar-router --no-pager
journalctl -u wowmax-stellar-router -n 50 --no-pager
curl -sS -m 5 http://127.0.0.1:8083/healthz | head -c 300
```

Typical causes: process crash (stack trace in journal), OOM (check
`journalctl -k | grep -i oom`), a bad deploy, or the port taken by a stale
process (`ss -ltnp | grep 8083`).

**Mitigate.** `systemctl restart wowmax-stellar-router`. The service builds its
first graph within seconds; `/healthz` reports `starting` until the first
build, then `ok`. If a fresh deploy caused the crash, roll back the working
tree to the last good commit and restart.

**Verify.** `stellar_router_up` returns to 1; venue panels repopulate on the
next probe cycle (≤ 90 s).

### Scenario 2 — DEX venue degraded or down (r2, warning)

**Symptoms.** r2 names the venue (`sdex` / `soroswap` / `phoenix` / `aqua`);
the `Venue serving` panel shows the drop; `Venue read errors` climbs.

**Diagnose.** The router's `/healthz` carries per-venue read results from the
last graph build:

```bash
curl -sS http://127.0.0.1:8083/healthz | python3 -m json.tool
curl -sS http://127.0.0.1:8083/rpcpool-stats | head -c 400
```

* `sdex` errors → Horizon problem (rate limit or outage).
* Soroban venues (`soroswap`/`phoenix`/`aqua`) erroring together → Soroban RPC
  provider trouble; check the pool stats for fallback/cooldown growth.
* A single Soroban venue erroring alone → that protocol's contracts or pools
  (check the venue's own status channels).

**Mitigate.** Provider-side incidents usually self-heal via the RPC pool
(primary → fallback). If the primary provider is hard-down for long, rotate
provider order in the router `.env` and restart. A single dead venue does not
stop routing: the winner simply comes from the remaining venues — this is the
graceful-degradation path by design.

**Verify.** `stellar_venue_serving{venue=...}` back to 1; read errors flat.

### Scenario 3 — Router quote p95 high (r7, warning)

**Symptoms.** r7 fires naming the slow probe pair; `Router /quote latency`
panel shows the p95 line above 20 s for that pair.

**Background.** `/quote` builds the routing graph live on every request (no
cache by policy — fresh reserves). Heavy pairs traverse more pools, so their
p95 is structurally higher; the 20 s threshold marks degradation, not the
normal spread. Real case 2026-08-02: p95 15.6 s was observed on secondary
pairs while the public gateway path answered in 0.43 s — the first threshold
(10 s) was too tight and was retuned to 20 s per pair.

**Diagnose.**

```bash
curl -sS http://127.0.0.1:8083/rpcpool-stats | python3 -m json.tool | head -30
journalctl -u wowmax-stellar-router --since "-30 min" --no-pager | tail -30
```

Rising `*_fallback_*` / `*_while_cooldown` counters → the primary RPC is slow
and requests ride the retry ladder. Flat pool stats with high build times →
venue-side slowness (Horizon or a specific AMM API).

**Mitigate.** Provider slowness: same rotation playbook as Scenario 2. If only
exotic probe pairs breach while user-facing pairs are fast, consider retuning
the probe pair list rather than the router.

### Scenario 4 — bridge-aggregator down (r4, critical)

**Symptoms.** r4 fires; all bridge probes fail at once; the swap UI loses
cross-chain quotes (falls back to "no route").

**Diagnose.**

```bash
ss -ltnp | grep 8085
tail -30 /tmp/bridge_svc.log
curl -sS -m 5 http://127.0.0.1:8085/health
```

**Mitigate.** Restart the process from the project directory:

```bash
cd /root/unibot/bridge-aggregator
pkill -f "tsx src/server.ts"; sleep 1
nohup npx tsx src/server.ts > /tmp/bridge_svc.log 2>&1 & disown
sleep 4 && curl -sS http://127.0.0.1:8085/health
```

The service holds no state; a restart is always safe. Unhandled SDK rejections
are trapped in-process (see `server.ts` top) precisely so one bridge SDK
cannot kill the others — if the log shows a crash loop from one adapter,
disable that adapter via its env switch and restart.

### Scenario 5 — Single bridge provider outage, transient (informational → r5)

**Symptoms.** One bridge's `quote_ok` drops to 0 for one or more pairs while
others keep quoting; the prober journal logs the provider's reason verbatim.
r5 fires only if **no** direct bridge serves a pair for 15 minutes.

**Real case, 2026-08-02 21:03 CEST.** Near Intents returned
`400 {"message":"No liquidity available","correlationId":"ae0affb4-…"}` for
`stellar:USDC->bsc:USDC`. Duration: under one probe cycle (≈ 3 min); recovered
without intervention; r5 correctly stayed silent (15 m threshold is the
anti-flap guard). The dashboard shows the dip as a one-sample step.

**Diagnose.**

```bash
journalctl -u stellar-prober --since "-30 min" --no-pager | grep "no-quote"
# Re-quote manually with the full reason:
curl -sS -X POST http://127.0.0.1:8085/bridge/quote -H 'content-type: application/json' \
  -d '{"fromChain":"stellar","fromToken":"USDC","toChain":"bsc","toToken":"USDC","amount":"100","sender":"<G...>","recipient":"<0x...>"}'
```

Keep the provider's `correlationId` — it is the support ticket key.

**Mitigate.** Nothing to do for short liquidity gaps: aggregation IS the
mitigation, the ranking simply serves the next best route. If one provider is
down for hours, raise it with the provider (correlationId attached) and note
it in the incident log.

### Scenario 6 — Bridge provider withdraws an architecture (r5 possible)

**Real case: Allbridge's Stellar routes stop after a security incident.**
On 2026-07-19 Allbridge Core was drained of ~$1.65M through a flash-loan
manipulation of its Solana stablecoin pool; the protocol paused, urged LPs to
withdraw, later resumed only the routes that do **not** rely on liquidity
pools, and announced it is phasing pool-based swaps out in favour of CCTP and
LayerZero routing. Allbridge's Stellar leg is pool-based (Soroban pool +
messenger contracts), so it fell on the disabled side of that line.

Our monitoring surfaced the symptom on 2026-08-02 as `allbridge → 400` on
USDC↔USDC in both directions — before we knew the cause. Isolated SDK probing
(full messenger matrix on SDK 3.32.0 **and** 3.32.1, both directions)
established:

* Core API `/receive-fee` rejects both pool messengers —
  `"Allbridge and Wormhole messengers are not supported"`;
* `getAmountToBeReceived` returns **0** for SRB routes;
* CCTP / CCTP_V2 / OFT / xReserve all answer `Such route does not support…`
  for Stellar;
* `transferTime` maps arrive empty for every destination.

A later check added one more signal: the SRB pool's 7-day APR is exactly
zero while its 30-day APR still carries pre-incident earnings — the pool is
not merely refusing quotes, it has stopped earning fees at all.

Conclusion: this is not a transient outage and not a bug on our side. It is a
provider retiring an entire route architecture. **Fix applied:** the
adapter's `supports()` declines any SRB leg, so the ranking reports a clean
`unsupported route` instead of a 400, and composite legs stop with it
(composites gate on `supports()`). Users saw no interruption: the ranking
served the remaining providers, including the composite `squid+wowmax` route,
in the same probe cycle.

**Re-enable path.** The trigger is not "Allbridge is back" but "Allbridge
serves Stellar on a non-pool architecture" — watch for a CCTP/LayerZero
Stellar deployment on their side. Verify with the messenger matrix probe
(`abdbg2.mjs`, run from the project directory): a live route shows a non-zero
`getAmountToBeReceived` and a real gas-fee option under some messenger. Only
then set `CC_ALLBRIDGE_SRB_ENABLED=true` and restart the aggregator.

**Known limitation while disabled:** composite (bridge + WOWMAX leg) coverage
for stable pairs shrinks, since those composites were built on Allbridge.

**General playbook when a provider's routes die:** reproduce with the
provider's own SDK in isolation (rule out our adapter), capture the exact API
error, then look outward — provider status posts and incident coverage often
explain in one sentence what an error message never will. Gate the route off
cleanly in `supports()` behind an env re-enable flag, write down the
re-enable *condition* rather than a vague "when it works again", and document
the case here.

### Scenario 7 — Public gateway e2e down (r6, critical)

**Symptoms.** r6 fires while the router itself is healthy (r1 silent): the
public path `Cloudflare → k8s api-gateway → Caddy (routing host) → router` is
broken somewhere in the middle. `Swagger /docs` stat may drop with it
(gateway-wide) or stay green (route-specific).

**Diagnose, outside-in.**

```bash
curl -sS -m 20 -o /dev/null -w 'public quote: %{http_code} %{time_total}s\n' \
  'https://api-gateway.wowmax.exchange/chains/100000148/quote?from=XLM&to=USDC&amount=100'
curl -sS -m 15 -o /dev/null -w '/docs: %{http_code}\n' https://api-gateway.wowmax.exchange/docs
curl -sS -m 5  -o /dev/null -w 'router direct: %{http_code}\n' 'http://127.0.0.1:8083/chains/100000148/quote?from=XLM&to=USDC&amount=100'
```

* Public 5xx/timeout + router direct 200 → three suspects between the edge and
  the router: the gateway pod (k8s dashboards: restarts, resource limits),
  Cloudflare, or Caddy on the routing host. Check Caddy first — it is the hop
  both `STELLAR_ROUTER_URL` and `BRIDGE_AGGREGATOR_URL` go through:

```bash
systemctl status caddy --no-pager
curl -sS -m 8 -o /dev/null -w 'via caddy: %{http_code}\n' https://stellar-router.wowmax.exchange/healthz
tail -20 /var/log/caddy/wowmax-stellar-router-access.log
```

* `/docs` down too → the whole gateway pod is unhealthy → k8s restart.
* Router direct also failing → this is Scenario 1, handle there.
* Caddy down → `systemctl restart caddy`; certificates renew automatically, no
  state to lose.

**Mitigate.** Restart the gateway deployment in k8s; the router side needs no
action. Escalate to the infrastructure owner if the cluster itself is the
problem.

### Scenario 8 — Monitoring is blind (r8, critical)

**Symptoms.** r8 (`prober down`) fires alone — by design r1–r7 stay silent on
missing data. Alternatively: dashboards freeze (no new points) without any
alert — that means VictoriaMetrics or Grafana's path to it is broken.

**Diagnose.**

```bash
systemctl status stellar-prober --no-pager
journalctl -u stellar-prober -n 30 --no-pager
docker ps --format '{{.Names}} {{.Status}}' | grep vmstellar
curl -sS -u "stellar:$(cut -d: -f2 /root/monitoring/.vmauth)" http://127.0.0.1:8428/health
```

**Mitigate.** Prober: `systemctl restart stellar-prober` (stateless).
VictoriaMetrics: `docker restart vmstellar` (data persists in
`/root/monitoring/vmdata`, retention 180 d). If the Grafana datasource errors
while VM is locally healthy, check the tunnel ingress
(`vm-stellar` hostname) and the datasource health in Grafana.

**Note.** While monitoring is blind the services usually keep working — after
recovery, review the gap window on the dashboards and the service journals for
anything missed.

## 4. Standard verification after any mitigation

1. The relevant `stellar_*` metric returns to its healthy value within one
   probe interval (90 s router / 180 s bridges / 120 s gateway).
2. The alert transitions to Resolved in Telegram.
3. `journalctl -u stellar-prober` shows clean probe cycles (no FAIL lines).
4. For bridge incidents: a manual `POST /bridge/quote` returns a winner and
   the expected `noQuotes` reasons only.

## 5. Change log

* 2026-08-05 (later) — Bridge source list corrected: the direct Axelar
  adapter is scoped away from Stellar (Squid carries Axelar ITS there) and its
  unsupported rows suppressed; probe pairs moved from BSC to the Ethereum
  corridor (`stellar:USDC->eth:USDT`, `stellar:XLM->eth:ETH`,
  `eth:USDC->stellar:USDC`).
* 2026-08-05 — Scenario 6 rewritten once the root cause was established: the
  Allbridge Stellar outage traces to the 2026-07-19 exploit and the
  protocol's subsequent retirement of pool-based routing, not to an isolated
  Stellar decision. Re-enable condition tightened accordingly.
* 2026-08-02 (later) — System map corrected: Caddy on the routing host
  publicly serves `stellar-router.wowmax.exchange` with a path split
  (`/bridge/*` → bridge-aggregator :8085, rest → router :8083); the k8s
  gateway consumes both services through it. Scenario 7 extended with the
  Caddy hop.
* 2026-08-02 — Runbook created. Alert set r1–r8 provisioned and live-tested
  (r7 fired on a real p95 breach; r8 tested via controlled prober stop; a real
  Near Intents liquidity transient and the Allbridge Stellar deprecation were
  caught and handled — Scenarios 5 and 6).
