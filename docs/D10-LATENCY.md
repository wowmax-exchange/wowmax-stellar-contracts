# WOWMAX Open Aggregation API — Public Latency Benchmark (D10)

SCF Build Award #43 — Deliverable 10, criterion: *p95 latency benchmark published*.

Run: 2026-08-02 21:52:36 CEST. Base URL: `https://api-gateway.wowmax.exchange`.

## Method

Sequential HTTP requests (concurrency 1, so the measurement adds no
self-induced queueing) from a Hetzner EU host over the public path —
Cloudflare edge -> Kubernetes api-gateway -> routing services. Each target
ran 5 discarded warm-up requests, then the counted sample. Non-200
responses are counted as failures and excluded from percentiles. Raw
timings: `bench_p95.json` next to this report.

The DEX quote is served with live reserves (no cache: every request hits
Horizon and Soroban RPC). The bridge quote fans out to every wired bridge
provider in parallel and waits for the slowest, so its latency is dominated
by third-party APIs — that is the cost of a complete ranking, by design;
the UI uses the fast Near-only first pass for instant display.

## Results (seconds)

| Endpoint | n | ok | fail | min | mean | p50 | p90 | p95 | p99 | max |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| DEX quote (Stellar XLM->USDC) | 200 | 199 | 1 | 0.360 | 0.413 | 0.383 | 0.405 | 0.611 | 1.193 | 1.281 |
| Bridge discovery (/bridge/chains) | 100 | 100 | 0 | 0.058 | 0.068 | 0.066 | 0.076 | 0.085 | 0.089 | 0.102 |
| Bridge quote fan-out (XLM->BSC USDT) | 25 | 25 | 0 | 1.267 | 1.940 | 1.525 | 3.610 | 4.568 | 5.863 | 6.258 |

## Continuous measurement

The same paths are measured continuously in production by the Stellar
monitoring prober (Prometheus histograms, p50/p95 panels on the internal
Grafana `Stellar` dashboards, alert on p95 degradation). This benchmark is
the point-in-time public snapshot; the histograms are the live view.

