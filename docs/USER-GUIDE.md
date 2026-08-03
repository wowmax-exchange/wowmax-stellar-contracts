# User guide — swapping and bridging on Stellar

This guide walks through the two things a user does in the WOWMAX app on
Stellar: swapping tokens on the DEX aggregator and moving value across chains
through the bridge aggregator. Everything is non-custodial — the app never
holds your funds or keys; every action ends with *your* wallet signing a
transaction it shows you first.

## 1. Swapping on Stellar

Open https://app.wowmax.exchange, switch the network selector to **Stellar**,
and connect a Stellar wallet (for example Freighter).

1. **Pick the pair and amount.** The quote appears with the expected output.
   Where routing beats the best single pool, the app shows the advantage —
   that is the aggregation premium you are getting over trading on one venue
   directly.
2. **Press Swap.** The engine re-checks the price at that exact moment
   against live chain state. Two outcomes:
   * the price still holds — your wallet opens with the transaction;
   * the market moved by more than 0.5% — instead of silently executing a
     worse trade, the app shows **“Price updated”** with the new number, and
     nothing happens until you press Swap again to confirm the new price.
3. **Check the wallet screen and sign.** The transaction carries a built-in
   minimum-output floor (slippage protection). The number in the wallet is
   close to the quote, not a scary worst case — and if the market moves past
   the floor between signing and inclusion, the transaction fails atomically
   rather than filling badly. A failed swap costs only the network fee; funds
   never leave in a partial state.
4. **Done.** The result lands in your wallet in the same ledger the
   transaction is included in — Stellar settles in a few seconds.

Behind the scenes each quote competes across two execution worlds — the
classic SDEX order books and the Soroban AMMs (Soroswap, Phoenix, Aquarius) —
and the better route wins. You do not need to care which one executed; the
guarantees above are identical. Details: [Routing logic](ROUTING.md).

## 2. Bridging to and from other chains

Open the cross-chain view, pick the source and destination chains and tokens,
enter the amount, and set the destination address.

1. **The first price appears instantly.** That is a fast single-provider
   pass, shown so you are never staring at a spinner.
2. **The full ranking follows a few seconds later.** Every wired bridge is
   quoted in parallel — plus *composite* routes that convert on the WOWMAX
   Stellar DEX first and bridge a stable leg after (often the best price for
   pairs like XLM → USDT). If something beats the instant price, the quote
   upgrades automatically.
3. **The route picker ("via …") shows the alternatives** with their net
   output, ETA and — where the provider exposes liquidity depth — a capacity
   badge (roughly how much the route can absorb). Pick the winner or any
   alternative you prefer.
4. **Execute and sign.** Depending on the provider model you will either
   sign a transaction from your wallet or send funds to a one-time deposit
   address shown by the app. Either way the payload is generated unsigned
   and shown to you; nothing moves without your signature.
5. **Track the transfer.** The app polls the transfer status until the
   destination transaction lands. If a provider cannot complete the transfer,
   the protocols used here refund to the source side — the status view will
   show `refunded` with the refund transaction.

If a bridge cannot serve your pair, the app does not hide it: the option is
absent from the picker for a stated reason (no liquidity right now,
unsupported pair, provider-side outage). The ranking only ever compares
routes that can actually execute.

## 3. Practical notes

* **Amount sizing.** Capacity badges exist for a reason: a route that is best
  for $100 may not absorb $50,000. For large transfers, compare the badge
  values and consider splitting.
* **ETAs are provider ETAs.** Near-instant for intent-based routes, minutes
  for message-passing bridges. The status view shows the live state either
  way.
* **Fees.** Every number the app shows is *net* — bridge fees and destination
  gas are already inside the quoted output. There are no hidden add-ons at
  signing time.
* **Testnet.** The router also runs on Stellar testnet with test assets for
  development sandboxes; the [developer quickstart](INTEGRATION.md) itself
  uses mainnet, since quotes and unsigned transactions cost nothing.

## 4. If something looks wrong

The stack is monitored around the clock (per-venue health, per-bridge
availability, latency, end-to-end public path) with alerting to the team —
see the [operations runbook](RUNBOOK.md) for how incidents are handled. If
you hit something the app cannot explain, open an issue in this repository
with the pair, the approximate time, and what you saw.
