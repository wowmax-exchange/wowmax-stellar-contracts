# The same deadline on three swap venues

The router can split a swap across venues that interpret the same deadline differently. [Kanatoko](https://crates.io/crates/kanatoko) runs this checkout’s router against saved Soroswap, Phoenix and Aquarius mainnet contracts and pool reserves.

| Scenario | Result |
| --- | --- |
| `deadline == now` | Soroswap rejects with error 503. The route through Phoenix and Aquarius settles. |
| Split 300 XLM across three venues | The trader receives the sum of the pool quotes. A minimum one stroop higher fails. |
| Merge the same pools with deadline zero | The trader receives the quoted total. The router finishes with zero XLM, USDC and AQUA balances. |

For a route that can include any of the three venues, use a future Unix timestamp or the router’s supported zero deadline. The tests replace the router executable locally and use the venue code and state in the [Protocol 29 fixtures](fixtures/).

[Read the Rust assertions](tests/mainnet.rs#L222).

## Reproduce locally

Use Rust 1.95.0 with the `wasm32v1-none` target. From the repo root, build the contracts, fetch the locked test dependencies once, then run the test offline.

```sh
cargo +1.95.0 build --locked --manifest-path contracts/Cargo.toml --target wasm32v1-none --profile release -p wowmax-stellar-router --target-dir integration-tests/mainnet/target
cargo +1.95.0 fetch --locked --manifest-path integration-tests/mainnet/Cargo.toml
cargo +1.95.0 test --locked --offline --manifest-path integration-tests/mainnet/Cargo.toml
```
