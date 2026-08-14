# Third-Party WebAssembly Blobs

Binary artifacts of external protocols, committed so that the adapter test
suites can run against real venue contracts instead of mocks. They are test
fixtures only: none is deployed by this project, and the executor contract
(`contracts/router`, the audit target) imports none of them — its suite runs
against an in-repo mock venue.

Upstream provenance is recorded per protocol below. `[FILL]` marks a value to
be confirmed against the upstream project before submission.

| Protocol | Upstream repository | Version / commit |
|---|---|---|
| Soroswap | [FILL] | [FILL] |
| Aquarius | [FILL] | [FILL] |
| Phoenix | [FILL] | [FILL] |
| Comet | [FILL] | [FILL] |

## Inventory

| File | Bytes | sha256 (first 16) | Imported by |
|---|---:|---|---|
| `contracts/adapters/aqua/aqua_contracts/soroban_fees_collector_contract.wasm` | 6,070 | `5f9633178daff14e` | **unreferenced** |
| `contracts/adapters/aqua/aqua_contracts/soroban_liquidity_pool_contract.wasm` | 45,087 | `549376178582fc69` | aqua_setup.rs |
| `contracts/adapters/aqua/aqua_contracts/soroban_liquidity_pool_liquidity_calculator_contract.wasm` | 20,943 | `75161be17f8f0286` | aqua_setup.rs |
| `contracts/adapters/aqua/aqua_contracts/soroban_liquidity_pool_plane_contract.wasm` | 7,370 | `3a35e48573a4aa30` | aqua_setup.rs |
| `contracts/adapters/aqua/aqua_contracts/soroban_liquidity_pool_router_contract.wasm` | 41,974 | `04b594a5f9c7ed52` | aqua_setup.rs, protocol_interface.rs |
| `contracts/adapters/aqua/aqua_contracts/soroban_liquidity_pool_stableswap_contract.wasm` | 55,953 | `00a7a17fcbf2c7e5` | aqua_setup.rs |
| `contracts/adapters/aqua/aqua_contracts/soroban_liquidity_pool_swap_router_contract.wasm` | 14,737 | `a09c7a9b3dbce8b7` | **unreferenced** |
| `contracts/adapters/aqua/aqua_contracts/soroban_locker_feed_contract.wasm` | 7,386 | `46bd97a0ccd582c7` | aqua_setup.rs |
| `contracts/adapters/aqua/aqua_contracts/soroban_token_contract.wasm` | 14,405 | `596ace8b85543647` | aqua_setup.rs |
| `contracts/adapters/comet/comet_contracts/comet_factory.wasm` | 2,502 | `bf7adb09076853eb` | comet_setup.rs |
| `contracts/adapters/comet/comet_contracts/comet_pool.wasm` | 29,046 | `8abc28913035c074` | comet_setup.rs, protocol_interface.rs |
| `contracts/adapters/phoenix/phoenix_contracts/phoenix_factory.wasm` | 27,141 | `c1f2b1bcbcfc5cfd` | phoenix_setup.rs |
| `contracts/adapters/phoenix/phoenix_contracts/phoenix_multihop.wasm` | 20,343 | `42067ea13010cf41` | phoenix_setup.rs, protocol_interface.rs |
| `contracts/adapters/phoenix/phoenix_contracts/phoenix_pool.wasm` | 42,241 | `7b8e336f2b6d855b` | phoenix_setup.rs |
| `contracts/adapters/phoenix/phoenix_contracts/phoenix_pool_stable.wasm` | 43,626 | `1cd5fdafe3c70d9d` | phoenix_setup.rs |
| `contracts/adapters/phoenix/phoenix_contracts/phoenix_stake.wasm` | 53,038 | `20dde039aace7e15` | phoenix_setup.rs |
| `contracts/adapters/phoenix/phoenix_contracts/phoenix_vesting.wasm` | 33,833 | `4f93ba7f4cdfe8ec` | **unreferenced** |
| `contracts/adapters/phoenix/phoenix_contracts/soroban_token_contract.wasm` | 7,163 | `c9fe635e9f7e1286` | phoenix_setup.rs |
| `contracts/adapters/soroswap/soroswap_contracts/soroban_token_contract.wasm` | 7,251 | `c8c70f9eca98862e` | soroswap_setup.rs |
| `contracts/adapters/soroswap/soroswap_contracts/soroswap_factory.wasm` | 11,167 | `b8f7c4289f9f8c18` | soroswap_setup.rs |
| `contracts/adapters/soroswap/soroswap_contracts/soroswap_pair.wasm` | 31,855 | `f25a763b8166ccde` | soroswap_setup.rs |
| `contracts/adapters/soroswap/soroswap_contracts/soroswap_router.optimized.wasm` | 34,253 | `4c3db3ebd2d6a2ab` | **unreferenced** |
| `contracts/adapters/soroswap/soroswap_contracts/soroswap_router.wasm` | 37,099 | `5f86422399da20b8` | protocol_interface.rs, soroswap_setup.rs |
| `contracts/router/aqua_contracts/soroban_fees_collector_contract.wasm` | 6,070 | `5f9633178daff14e` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_liquidity_pool_contract.wasm` | 45,087 | `549376178582fc69` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_liquidity_pool_liquidity_calculator_contract.wasm` | 20,943 | `75161be17f8f0286` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_liquidity_pool_plane_contract.wasm` | 7,370 | `3a35e48573a4aa30` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_liquidity_pool_router_contract.wasm` | 41,974 | `04b594a5f9c7ed52` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_liquidity_pool_stableswap_contract.wasm` | 55,953 | `00a7a17fcbf2c7e5` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_liquidity_pool_swap_router_contract.wasm` | 14,737 | `a09c7a9b3dbce8b7` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_locker_feed_contract.wasm` | 7,386 | `46bd97a0ccd582c7` | **unreferenced** |
| `contracts/router/aqua_contracts/soroban_token_contract.wasm` | 14,405 | `596ace8b85543647` | **unreferenced** |
| `contracts/router/comet_contracts/comet_factory.wasm` | 2,502 | `bf7adb09076853eb` | **unreferenced** |
| `contracts/router/comet_contracts/comet_pool.wasm` | 29,046 | `8abc28913035c074` | **unreferenced** |
| `contracts/router/phoenix_contracts/phoenix_factory.wasm` | 27,141 | `c1f2b1bcbcfc5cfd` | **unreferenced** |
| `contracts/router/phoenix_contracts/phoenix_multihop.wasm` | 20,343 | `42067ea13010cf41` | **unreferenced** |
| `contracts/router/phoenix_contracts/phoenix_pool.wasm` | 42,241 | `7b8e336f2b6d855b` | **unreferenced** |
| `contracts/router/phoenix_contracts/phoenix_pool_stable.wasm` | 43,626 | `1cd5fdafe3c70d9d` | **unreferenced** |
| `contracts/router/phoenix_contracts/phoenix_stake.wasm` | 53,038 | `20dde039aace7e15` | **unreferenced** |
| `contracts/router/phoenix_contracts/phoenix_vesting.wasm` | 33,833 | `4f93ba7f4cdfe8ec` | **unreferenced** |
| `contracts/router/phoenix_contracts/soroban_token_contract.wasm` | 7,163 | `c9fe635e9f7e1286` | **unreferenced** |
| `contracts/router/soroswap_contracts/soroban_token_contract.wasm` | 7,251 | `c8c70f9eca98862e` | **unreferenced** |
| `contracts/router/soroswap_contracts/soroswap_factory.wasm` | 11,167 | `b8f7c4289f9f8c18` | **unreferenced** |
| `contracts/router/soroswap_contracts/soroswap_pair.wasm` | 31,855 | `f25a763b8166ccde` | **unreferenced** |
| `contracts/router/soroswap_contracts/soroswap_router.optimized.wasm` | 34,253 | `4c3db3ebd2d6a2ab` | **unreferenced** |
| `contracts/router/soroswap_contracts/soroswap_router.wasm` | 37,099 | `5f86422399da20b8` | **unreferenced** |

Full digests are reproducible with:

```bash
find contracts -name '*.wasm' -not -path '*/target/*' | sort | xargs sha256sum
```
