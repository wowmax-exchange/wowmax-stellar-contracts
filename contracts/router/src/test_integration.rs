#![cfg(test)]
//! Executor tests against REAL venue contracts.
//!
//! The suite in `test.rs` drives the executor against an in-repo mock that is
//! built to misbehave — it can over-report its output, refuse to consume the
//! input, consume only part of it, or side-transfer an asset the route never
//! declared. Those cases prove the hostile-venue guards, and a real pool
//! cannot exercise them because a real pool behaves.
//!
//! This file proves the complementary half: that the executor drives a
//! genuine AMM correctly — real Soroswap factory, real pair contracts, real
//! router, real reserves — across a single hop, a two-hop chain, a split, the
//! slippage guard, and a merge DAG.

extern crate std;

use soroban_sdk::{vec, Address, BytesN, Env, Vec};
use test_utils::soroswap_setup::SoroswapTest;

use crate::{Fill, Hop, Stage, Strand, WowmaxAggregator, WowmaxAggregatorClient};

const SOROSWAP: u32 = 0;

/// Aquarius fields are unused for a Soroswap hop; the plan carries harmless
/// placeholders, exactly as the off-chain planner emits them.
fn hop(
    env: &Env,
    pool: &Address,
    router: &Address,
    token_in: &Address,
    token_out: &Address,
) -> Hop {
    Hop {
        venue: SOROSWAP,
        pool: pool.clone(),
        token_in: token_in.clone(),
        token_out: token_out.clone(),
        aqua_router: router.clone(),
        aqua_pool_tokens: Vec::new(env),
        aqua_pool_index: BytesN::from_array(env, &[0u8; 32]),
        soroswap_router: router.clone(),
        soroswap_path: vec![env, token_in.clone(), token_out.clone()],
    }
}

fn fill(
    env: &Env,
    pool: &Address,
    router: &Address,
    stage_token: &Address,
    token_out: &Address,
    parts: u32,
) -> Fill {
    Fill {
        venue: SOROSWAP,
        pool: pool.clone(),
        token_out: token_out.clone(),
        parts,
        aqua_router: router.clone(),
        aqua_pool_tokens: Vec::new(env),
        aqua_pool_index: BytesN::from_array(env, &[0u8; 32]),
        soroswap_router: router.clone(),
        soroswap_path: vec![env, stage_token.clone(), token_out.clone()],
    }
}

struct Fixture<'a> {
    t: SoroswapTest<'a>,
    client: WowmaxAggregatorClient<'a>,
    executor: Address,
    deadline: u64,
}

fn setup<'a>() -> Fixture<'a> {
    let t = SoroswapTest::soroswap_setup();
    let executor = t.env.register(WowmaxAggregator, ());
    let client = WowmaxAggregatorClient::new(&t.env, &executor);
    let deadline = t.ledger_timestamp + 900;
    Fixture {
        t,
        client,
        executor,
        deadline,
    }
}

/// The executor must never hold anything once a call returns.
fn assert_executor_drained(f: &Fixture) {
    assert_eq!(f.t.token_0.balance(&f.executor), 0, "token_0 retained");
    assert_eq!(f.t.token_1.balance(&f.executor), 0, "token_1 retained");
    assert_eq!(f.t.token_2.balance(&f.executor), 0, "token_2 retained");
}

#[test]
fn real_pool_single_hop() {
    let f = setup();
    let amount_in: i128 = 1_000_000_000_000_000;
    let pool = f.t.pair(&f.t.token_0.address, &f.t.token_1.address);

    let before_in = f.t.token_0.balance(&f.t.user);
    let before_out = f.t.token_1.balance(&f.t.user);

    let plan = vec![
        &f.t.env,
        Strand {
            parts: 1,
            hops: vec![
                &f.t.env,
                hop(
                    &f.t.env,
                    &pool,
                    &f.t.router_contract.address,
                    &f.t.token_0.address,
                    &f.t.token_1.address,
                ),
            ],
        },
    ];

    let out = f.client.swap(
        &f.t.user,
        &f.t.token_0.address,
        &f.t.token_1.address,
        &amount_in,
        &0,
        &f.deadline,
        &plan,
    );

    assert!(out > 0, "no output from a real pool");
    assert_eq!(f.t.token_0.balance(&f.t.user), before_in - amount_in);
    assert_eq!(f.t.token_1.balance(&f.t.user), before_out + out);
    assert_executor_drained(&f);
}

#[test]
fn real_pools_two_hop_chain() {
    let f = setup();
    let amount_in: i128 = 1_000_000_000_000_000;
    let pool_01 = f.t.pair(&f.t.token_0.address, &f.t.token_1.address);
    let pool_12 = f.t.pair(&f.t.token_1.address, &f.t.token_2.address);

    let before_out = f.t.token_2.balance(&f.t.user);

    let plan = vec![
        &f.t.env,
        Strand {
            parts: 1,
            hops: vec![
                &f.t.env,
                hop(
                    &f.t.env,
                    &pool_01,
                    &f.t.router_contract.address,
                    &f.t.token_0.address,
                    &f.t.token_1.address,
                ),
                hop(
                    &f.t.env,
                    &pool_12,
                    &f.t.router_contract.address,
                    &f.t.token_1.address,
                    &f.t.token_2.address,
                ),
            ],
        },
    ];

    let out = f.client.swap(
        &f.t.user,
        &f.t.token_0.address,
        &f.t.token_2.address,
        &amount_in,
        &0,
        &f.deadline,
        &plan,
    );

    assert!(out > 0, "two-hop chain produced no output");
    assert_eq!(f.t.token_2.balance(&f.t.user), before_out + out);
    // The intermediate token must not linger in an immutable contract.
    assert_executor_drained(&f);
}

#[test]
fn real_pool_split_across_two_strands() {
    let f = setup();
    let amount_in: i128 = 1_000_000_000_000_000;
    let pool = f.t.pair(&f.t.token_0.address, &f.t.token_1.address);

    let before_in = f.t.token_0.balance(&f.t.user);
    let before_out = f.t.token_1.balance(&f.t.user);

    let one = hop(
        &f.t.env,
        &pool,
        &f.t.router_contract.address,
        &f.t.token_0.address,
        &f.t.token_1.address,
    );
    let plan = vec![
        &f.t.env,
        Strand {
            parts: 1,
            hops: vec![&f.t.env, one.clone()],
        },
        Strand {
            parts: 3,
            hops: vec![&f.t.env, one],
        },
    ];

    let out = f.client.swap(
        &f.t.user,
        &f.t.token_0.address,
        &f.t.token_1.address,
        &amount_in,
        &0,
        &f.deadline,
        &plan,
    );

    assert!(out > 0, "split produced no output");
    // The whole input is spent, remainder included: the last strand takes
    // what integer division left behind.
    assert_eq!(f.t.token_0.balance(&f.t.user), before_in - amount_in);
    assert_eq!(f.t.token_1.balance(&f.t.user), before_out + out);
    assert_executor_drained(&f);
}

#[test]
#[should_panic(expected = "amount_out_min not met")]
fn real_pool_slippage_guard_reverts() {
    let f = setup();
    let amount_in: i128 = 1_000_000_000_000_000;
    let pool = f.t.pair(&f.t.token_0.address, &f.t.token_1.address);

    let plan = vec![
        &f.t.env,
        Strand {
            parts: 1,
            hops: vec![
                &f.t.env,
                hop(
                    &f.t.env,
                    &pool,
                    &f.t.router_contract.address,
                    &f.t.token_0.address,
                    &f.t.token_1.address,
                ),
            ],
        },
    ];

    // Far above anything this pool can deliver for that input.
    let impossible_min: i128 = amount_in * 1_000;
    f.client.swap(
        &f.t.user,
        &f.t.token_0.address,
        &f.t.token_1.address,
        &amount_in,
        &impossible_min,
        &f.deadline,
        &plan,
    );
}

#[test]
fn real_pools_merge_two_stages() {
    let f = setup();
    let amount_in: i128 = 1_000_000_000_000_000;
    let pool_01 = f.t.pair(&f.t.token_0.address, &f.t.token_1.address);
    let pool_12 = f.t.pair(&f.t.token_1.address, &f.t.token_2.address);

    let before_out = f.t.token_2.balance(&f.t.user);

    let stages = vec![
        &f.t.env,
        Stage {
            token: f.t.token_0.address.clone(),
            fills: vec![
                &f.t.env,
                fill(
                    &f.t.env,
                    &pool_01,
                    &f.t.router_contract.address,
                    &f.t.token_0.address,
                    &f.t.token_1.address,
                    1,
                ),
            ],
        },
        Stage {
            token: f.t.token_1.address.clone(),
            fills: vec![
                &f.t.env,
                fill(
                    &f.t.env,
                    &pool_12,
                    &f.t.router_contract.address,
                    &f.t.token_1.address,
                    &f.t.token_2.address,
                    1,
                ),
            ],
        },
    ];

    let out = f.client.swap_merge(
        &f.t.user,
        &f.t.token_0.address,
        &f.t.token_2.address,
        &amount_in,
        &0,
        &f.deadline,
        &stages,
    );

    assert!(out > 0, "merge produced no output");
    assert_eq!(f.t.token_2.balance(&f.t.user), before_out + out);
    assert_executor_drained(&f);
}
