# Developer integration — quickstart

Goal: from an empty directory to your first live WOWMAX quote in under five
minutes, and to a complete integration path — quote → unsigned transaction →
(optionally) signed swap on-chain — in under fifteen. No API key, no funds
required for everything except the optional final broadcast.

Two ways in: the TypeScript SDK (`@wowmax/sdk`) or plain REST. Both talk to
the same public gateway: `https://api-gateway.wowmax.exchange`
(Swagger: [/docs](https://api-gateway.wowmax.exchange/docs), OpenAPI:
[/docs-json](https://api-gateway.wowmax.exchange/docs-json)).

## 1. First quote (≈ 3 minutes)

```bash
mkdir wowmax-quickstart && cd wowmax-quickstart
npm init -y && npm i @wowmax/sdk
```

`quote.mjs`:

```js
import { WowmaxClient, STELLAR_CHAIN_ID, fromBaseUnits } from '@wowmax/sdk';

const wowmax = new WowmaxClient();

const q = await wowmax.getQuote(STELLAR_CHAIN_ID, {
  from: 'XLM',
  to: 'USDC',
  amount: '100',
});

console.log('100 XLM ->', fromBaseUnits(q.amountOut, 7), 'USDC');
console.log('routes:', JSON.stringify(q.routes).slice(0, 200));
```

```bash
node quote.mjs
```

You just used the same engine the app uses: the quote competed across the
SDEX order books and the Soroban AMMs (Soroswap, Phoenix, Aquarius) with live
reserves, and you got the winner. Token identifiers are `XLM` (or `native`)
and `CODE:ISSUER` for issued assets; well-known codes like `USDC`, `AQUA`,
`EURC` resolve by symbol.

REST equivalent, no SDK:

```bash
curl 'https://api-gateway.wowmax.exchange/chains/100000148/quote?from=XLM&to=USDC&amount=100'
```

`100000148` is the synthetic Stellar chain id used across the WOWMAX API.

## 2. Unsigned swap transaction (≈ 3 minutes)

The swap endpoint returns a **complete unsigned XDR** built for your account.
Use any funded Stellar address you control (your wallet address is fine —
building is read-only and free):

```js
import { WowmaxClient, STELLAR_CHAIN_ID } from '@wowmax/sdk';

const wowmax = new WowmaxClient();
const ACCOUNT = 'G...your-stellar-address';

const swap = await wowmax.getSwap(STELLAR_CHAIN_ID, {
  from: 'XLM',
  to: 'USDC',
  amount: '5',
  account: ACCOUNT,
  slippage: 0.5,
});

console.log('mode:', swap.mode);          // 'classic' | 'soroban'
console.log('unsigned XDR:', swap.xdr.slice(0, 80), '...');
```

Nothing has moved: WOWMAX never signs and never sees a key. One behaviour you
must handle in a real integration: if the market moved more than 0.5% between
quote and build, the response is a `rate_updated` signal with the fresh
number instead of an XDR — re-confirm with your user and call again. Never
silently retry a worse price.

## 3. Optional: execute it (≈ 5 minutes, costs a real swap)

Sign the XDR with any Stellar tooling and submit. With the official SDK:

```bash
npm i @stellar/stellar-sdk
```

```js
import { Keypair, TransactionBuilder, Networks } from '@stellar/stellar-sdk';

const tx = TransactionBuilder.fromXDR(swap.xdr, Networks.PUBLIC);
tx.sign(Keypair.fromSecret(process.env.STELLAR_SECRET));
const res = await fetch('https://horizon.stellar.org/transactions', {
  method: 'POST',
  headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
  body: 'tx=' + encodeURIComponent(tx.toXDR()),
}).then(r => r.json());
console.log(res.successful ? 'swapped: ' + res.hash : res);
```

The transaction carries a minimum-output floor, so if the market moves past
your slippage between signing and inclusion it fails atomically — a failed
attempt costs only the network fee.

## 4. Cross-chain in the same client (≈ 3 minutes)

The bridge aggregation surface lives on the same gateway under
`/crosschain/v0/bridge/*` and in the same SDK:

```js
const bq = await wowmax.bridgeQuote({
  fromChain: 'stellar',
  fromToken: 'XLM',
  toChain: 'bsc',
  toToken: 'USDT',
  amount: '100',
  sender: 'G...your-stellar-address',
  recipient: '0x...your-evm-address',
});

console.log('winner:', bq.winner?.bridge, '->', bq.winner?.amountOut, 'USDT');
for (const row of bq.merged) console.log(row.rank, row.kind, row.bridge, row.netUsd);
for (const n of bq.direct.noQuotes) console.log('declined:', n.bridge, '—', n.reason);
```

One call quotes every wired bridge in parallel plus composite
(bridge + WOWMAX Stellar leg) routes, ranked on net USD. `bridgeExecute`
returns the unsigned payload for the chosen route; `bridgeStatus` tracks the
transfer to completion. The fan-out waits for the slowest provider — the SDK
uses a 60 s timeout for these calls (the DEX endpoints use 15 s).

## 5. Where to go next

* Full endpoint reference: [Swagger](https://api-gateway.wowmax.exchange/docs)
  — every path here, try-it-out enabled.
* [Routing logic](ROUTING.md) — what the engine guarantees, `rate_updated`
  and slippage semantics you should surface to users.
* Published latency: [benchmark](D10-LATENCY.md) — plan your UI around
  DEX-quote p95 ≈ 0.6 s and bridge fan-out p95 ≈ 4.6 s (show the fast pass
  first, upgrade when the ranking lands — that is what the WOWMAX app does).
* AI-agent integration: the same aggregation is exposed as MCP tools at
  `https://app.wowmax.exchange/mcp` for agent frameworks that speak the
  Model Context Protocol.
* Stuck? Open an issue in this repository.
