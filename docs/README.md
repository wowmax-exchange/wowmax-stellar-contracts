# WOWMAX on Stellar — Documentation

WOWMAX brings full DEX aggregation and multi-bridge cross-chain routing to
Stellar as a first-class network: classic SDEX order books and Soroban AMMs
(Soroswap, Phoenix, Aquarius) behind one quote, plus bridge aggregation across
Near Intents, Allbridge, Axelar and Squid with composite routes through the
Stellar DEX leg. Everything is non-custodial: the APIs return unsigned
transactions; keys, signing and broadcast stay with the user.

Built under Stellar Community Fund Build Award #43.

## Documentation

| Section | What it covers |
| --- | --- |
| [Architecture](ARCHITECTURE.md) | Components, data flow, execution model, non-custodial guarantees |
| [Routing logic](ROUTING.md) | How a quote is computed: two execution layers, winner selection, price guards |
| [User guide](USER-GUIDE.md) | Swapping and bridging in the WOWMAX app, step by step |
| [Developer integration](INTEGRATION.md) | REST + TypeScript SDK quickstart — first quote in ~3 minutes, full non-custodial cycle under 15 |
| [Contribution model](CONTRIBUTING.md) | Adding a DEX venue or a bridge adapter, code and security expectations |
| [Operations runbook](RUNBOOK.md) | Production monitoring, alerting, incident scenarios |
| [Latency benchmark](D10-LATENCY.md) | Public p50/p95/p99 measurements of the live API |
| [D1 engine report](REPORT.md) | The original routing-engine deliverable report |
| [D4 health report](D4-REPORT.md) | Protocol health monitoring deliverable report |

## Live surfaces

| Surface | URL |
| --- | --- |
| Swap app | https://app.wowmax.exchange |
| Live monitoring (public dashboards) | https://grafana.wowmax.exchange |
| Public API (Swagger) | https://api-gateway.wowmax.exchange/docs |
| OpenAPI JSON | https://api-gateway.wowmax.exchange/docs-json |
| TypeScript SDK | https://www.npmjs.com/package/@wowmax/sdk |
| Execution contracts (this repo) | https://github.com/wowmax-exchange/wowmax-stellar-contracts |
| Bridge aggregator source | https://github.com/wowmax-exchange/bridge-aggregator |
| Benchmark harness | https://github.com/wowmax-exchange/wowmax-benchmarks |

## Support

Open an issue in this repository, or reach the team through the channels on
https://wowmax.exchange.
