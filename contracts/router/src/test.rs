#![cfg(test)]
//! Router test suite.
//!
//! These tests exercise the plan executor itself — the splitter, the hop
//! chain, token continuity and the balance-delta accounting — against a
//! mock venue that speaks the Soroswap router interface. Auth is mocked
//! (`mock_all_auths`), so what is under test is the executor's arithmetic
//! and its guards, not the auth subtree (that is proven on mainnet).
//!
//! The mock deliberately supports lying: `report_bonus` makes it report a
//! larger output than it actually delivers, which is how `inflated_report_
//! is_ignored` proves the contract trusts its own balance delta.

extern crate std;

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{
    contract, contractimpl, symbol_short, token, vec, Address, BytesN, Env, Vec,
};

use crate::{Fill, Hop, Stage, Strand, WowmaxAggregator, WowmaxAggregatorClient};

// ----------------------------- mock venue -----------------------------

#[contract]
pub struct MockVenue;

#[contractimpl]
impl MockVenue {
    /// `num`/`den` is the exchange rate, `report_bonus` is added to the
    /// REPORTED output without being delivered.
    pub fn init(env: Env, pool: Address, num: i128, den: i128, report_bonus: i128) {
        env.storage().instance().set(&symbol_short!("pool"), &pool);
        env.storage().instance().set(&symbol_short!("num"), &num);
        env.storage().instance().set(&symbol_short!("den"), &den);
        env.storage().instance().set(&symbol_short!("bonus"), &report_bonus);
        env.storage().instance().set(&symbol_short!("mode"), &0u32);
    }

    /// The deadline the venue was handed on its most recent swap.
    pub fn last_deadline(env: Env) -> u64 {
        env.storage().instance().get(&symbol_short!("lastdl")).unwrap_or(0u64)
    }

    /// Hostile behaviour switch.
    /// 0 = honest, 1 = deliver output but never pull the input,
    /// 2 = pull only part of the authorized input.
    pub fn set_mode(env: Env, mode: u32) {
        env.storage().instance().set(&symbol_short!("mode"), &mode);
    }

    /// When a swap's declared output equals `trigger_out`, also push
    /// `side_amount` of `side_token` to the caller out of the venue's own
    /// balance — a rebate, an extra settlement leg, or plain misbehaviour.
    pub fn set_side_output(env: Env, trigger_out: Address, side_token: Address, side_amount: i128) {
        env.storage().instance().set(&symbol_short!("trigout"), &trigger_out);
        env.storage().instance().set(&symbol_short!("sidetkn"), &side_token);
        env.storage().instance().set(&symbol_short!("sideamt"), &side_amount);
    }

    pub fn swap_exact_tokens_for_tokens(
        env: Env,
        amount_in: i128,
        _amount_out_min: i128,
        path: Vec<Address>,
        to: Address,
        deadline: u64,
    ) -> Vec<i128> {
        env.storage().instance().set(&symbol_short!("lastdl"), &deadline);
        let pool: Address = env.storage().instance().get(&symbol_short!("pool")).unwrap();
        let num: i128 = env.storage().instance().get(&symbol_short!("num")).unwrap();
        let den: i128 = env.storage().instance().get(&symbol_short!("den")).unwrap();
        let bonus: i128 = env.storage().instance().get(&symbol_short!("bonus")).unwrap();

        let token_in = path.get(0).unwrap();
        let token_out = path.last().unwrap();

        let mode: u32 = env
            .storage()
            .instance()
            .get(&symbol_short!("mode"))
            .unwrap_or(0u32);

        // Pull the input exactly as the real router does — unless we are
        // told to misbehave.
        if mode == 0 {
            token::Client::new(&env, &token_in).transfer(&to, &pool, &amount_in);
        } else if mode == 2 {
            let partial = amount_in / 2;
            token::Client::new(&env, &token_in).transfer(&to, &pool, &partial);
        }

        let delivered = amount_in * num / den;
        token::StellarAssetClient::new(&env, &token_out).mint(&to, &delivered);

        let trigger: Option<Address> = env.storage().instance().get(&symbol_short!("trigout"));
        if trigger == Some(token_out.clone()) {
            let side_token: Address =
                env.storage().instance().get(&symbol_short!("sidetkn")).unwrap();
            let side_amount: i128 =
                env.storage().instance().get(&symbol_short!("sideamt")).unwrap();
            let me = env.current_contract_address();
            token::Client::new(&env, &side_token).transfer(&me, &to, &side_amount);
        }

        vec![&env, amount_in, delivered + bonus]
    }
}

// ------------------------------- harness ------------------------------

struct Fixture {
    env: Env,
    client: WowmaxAggregatorClient<'static>,
    user: Address,
    token_a: Address,
    token_b: Address,
    venue: Address,
    /// The address the venue pulls token_in INTO. It must match the
    /// `pool` field of the Hop, because that is exactly what the
    /// contract pre-authorizes via `authorize_as_current_contract`.
    pool: Address,
}

fn setup(num: i128, den: i128, report_bonus: i128) -> Fixture {
    let env = Env::default();
    // The mock venue mints token_out to the aggregator deep inside the
    // call stack (SAC admin auth), which plain `mock_all_auths` rejects
    // as "not tied to the root invocation". The auth subtree itself is
    // proven on mainnet; here we test the executor's arithmetic.
    env.mock_all_auths_allowing_non_root_auth();
    env.ledger().set_timestamp(1_000_000);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let pool = Address::generate(&env);

    let token_a = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let token_b = env.register_stellar_asset_contract_v2(admin.clone()).address();

    let venue = env.register(MockVenue, ());
    MockVenueClient::new(&env, &venue).init(&pool, &num, &den, &report_bonus);

    let contract_id = env.register(WowmaxAggregator, ());
    let client = WowmaxAggregatorClient::new(&env, &contract_id);

    token::StellarAssetClient::new(&env, &token_a).mint(&user, &1_000_000i128);

    Fixture { env, client, user, token_a, token_b, venue, pool }
}

/// A single-hop strand on the mock venue (venue = 0, Soroswap interface).
fn hop(
    env: &Env,
    venue: &Address,
    pool: &Address,
    token_in: &Address,
    token_out: &Address,
) -> Hop {
    Hop {
        venue: 0,
        pool: pool.clone(),
        token_in: token_in.clone(),
        token_out: token_out.clone(),
        aqua_router: venue.clone(),
        aqua_pool_tokens: vec![env],
        aqua_pool_index: BytesN::from_array(env, &[0u8; 32]),
        soroswap_router: venue.clone(),
        soroswap_path: vec![env, token_in.clone(), token_out.clone()],
    }
}

fn strand(parts: u32, hops: Vec<Hop>) -> Strand {
    Strand { parts, hops }
}

// -------------------------------- tests -------------------------------

#[test]
fn single_strand_happy_path() {
    let f = setup(2, 1, 0); // 2x
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];

    let out = f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1_900i128, &2_000_000u64, &plan,
    );

    assert_eq!(out, 2_000);
    assert_eq!(token::Client::new(&f.env, &f.token_b).balance(&f.user), 2_000);
}

#[test]
fn split_respects_parts_and_remainder() {
    let f = setup(1, 1, 0); // 1:1, so outputs mirror the split exactly
    let h = hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b);
    let plan = vec![
        &f.env,
        strand(3, vec![&f.env, h.clone()]),
        strand(1, vec![&f.env, h.clone()]),
    ];

    // 1001 split 3:1 -> floor(1001*3/4) = 750, remainder 251.
    let out = f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_001i128, &1_001i128, &2_000_000u64, &plan,
    );

    assert_eq!(out, 1_001, "the split must sum to amount_in exactly");
}

#[test]
fn inflated_report_is_ignored() {
    // The venue reports +5_000 more than it delivers. The contract must
    // forward the delivered amount, not the reported one.
    let f = setup(1, 1, 5_000);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];

    let out = f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1_000i128, &2_000_000u64, &plan,
    );

    assert_eq!(out, 1_000, "output must come from the balance delta");
    assert_eq!(token::Client::new(&f.env, &f.token_b).balance(&f.user), 1_000);
}

#[test]
#[should_panic(expected = "amount_out_min not met")]
fn slippage_guard_reverts() {
    let f = setup(1, 1, 0);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1_001i128, &2_000_000u64, &plan,
    );
}

#[test]
#[should_panic(expected = "strand must start at token_in")]
fn plan_must_start_at_token_in() {
    let f = setup(1, 1, 0);
    // Hop claims token_b as its input while the call declares token_a.
    let bad = hop(&f.env, &f.venue, &f.pool, &f.token_b, &f.token_a);
    let plan = vec![&f.env, strand(1, vec![&f.env, bad])];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
}

#[test]
#[should_panic(expected = "strand must end at token_out")]
fn plan_must_end_at_token_out() {
    let f = setup(1, 1, 0);
    let token_c = env_token(&f.env);
    let bad = hop(&f.env, &f.venue, &f.pool, &f.token_a, &token_c);
    let plan = vec![&f.env, strand(1, vec![&f.env, bad])];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
}

#[test]
#[should_panic(expected = "empty plan")]
fn empty_plan_reverts() {
    let f = setup(1, 1, 0);
    let plan: Vec<Strand> = vec![&f.env];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
}

#[test]
#[should_panic(expected = "amount_in must be positive")]
fn non_positive_amount_reverts() {
    let f = setup(1, 1, 0);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &0i128, &0i128, &2_000_000u64, &plan,
    );
}

#[test]
#[should_panic(expected = "token_in equals token_out")]
fn self_swap_reverts() {
    let f = setup(1, 1, 0);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_a)]),
    ];
    f.client.swap(
        &f.user, &f.token_a, &f.token_a, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
}

#[test]
#[should_panic(expected = "deadline passed")]
fn expired_deadline_reverts() {
    let f = setup(1, 1, 0);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];
    // ledger timestamp is 1_000_000 in the fixture
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &999_999u64, &plan,
    );
}

/// Not a pass/fail test — it prints the CPU/memory budget consumed by a
/// deliberately heavy plan (4 strands x 2 hops = 8 venue calls) so the
/// cost of balance-delta accounting can be compared against the Soroban
/// per-transaction ceiling. Run with `cargo test -- --nocapture`.
#[test]
fn budget_heavy_plan() {
    let f = setup(1, 1, 0);
    let token_c = env_token(&f.env);
    let chain = vec![
        &f.env,
        hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b),
        hop(&f.env, &f.venue, &f.pool, &f.token_b, &token_c),
    ];
    let plan = vec![
        &f.env,
        strand(1, chain.clone()),
        strand(1, chain.clone()),
        strand(1, chain.clone()),
        strand(1, chain.clone()),
    ];

    let out = f.client.swap(
        &f.user, &f.token_a, &token_c, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
    assert_eq!(out, 1_000);

    let budget = f.env.cost_estimate().budget();
    std::println!("=== heavy plan: 4 strands x 2 hops (8 venue calls) ===");
    std::println!("CPU instructions: {}", budget.cpu_instruction_cost());
    std::println!("memory bytes:     {}", budget.memory_bytes_cost());
}

// ---------------------- merge / provenance / hostile ----------------------

/// A single-hop fill on the mock venue (venue = 0, Soroswap interface).
/// `token_in` is the stage's token: the contract passes it separately, but
/// the mock reads both ends of the swap out of `soroswap_path`.
fn fill(
    env: &Env,
    venue: &Address,
    pool: &Address,
    token_in: &Address,
    token_out: &Address,
    parts: u32,
) -> Fill {
    Fill {
        venue: 0,
        pool: pool.clone(),
        token_out: token_out.clone(),
        parts,
        aqua_router: venue.clone(),
        aqua_pool_tokens: vec![env],
        aqua_pool_index: BytesN::from_array(env, &[0u8; 32]),
        soroswap_router: venue.clone(),
        soroswap_path: vec![env, token_in.clone(), token_out.clone()],
    }
}

#[test]
fn merge_happy_path_two_stages() {
    let f = setup(1, 1, 0);
    let token_c = env_token(&f.env);
    let stages = vec![
        &f.env,
        Stage {
            token: f.token_a.clone(),
            fills: vec![&f.env, fill(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b, 1)],
        },
        Stage {
            token: f.token_b.clone(),
            fills: vec![&f.env, fill(&f.env, &f.venue, &f.pool, &f.token_b, &token_c, 1)],
        },
    ];

    let out = f.client.swap_merge(
        &f.user, &f.token_a, &token_c, &1_000i128, &1i128, &2_000_000u64, &stages,
    );
    assert_eq!(out, 1_000);
    assert_eq!(token::Client::new(&f.env, &token_c).balance(&f.user), 1_000);
}

/// H-01 regression. The contract is pre-loaded with token V. A plan that
/// names V as a stage token must not be able to spend it.
#[test]
#[should_panic(expected = "stage token has no current-call balance")]
fn merge_cannot_spend_pre_existing_balance() {
    let f = setup(1, 1, 0);
    let token_v = env_token(&f.env);
    let contract_addr = f.client.address.clone();
    // Someone's tokens are sitting in the contract.
    token::StellarAssetClient::new(&f.env, &token_v).mint(&contract_addr, &500_000i128);

    let stages = vec![
        &f.env,
        // Stage 1 tries to sweep the pre-existing V.
        Stage {
            token: token_v.clone(),
            fills: vec![&f.env, fill(&f.env, &f.venue, &f.pool, &token_v, &f.token_b, 1)],
        },
        Stage {
            token: f.token_a.clone(),
            fills: vec![&f.env, fill(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b, 1)],
        },
    ];

    f.client.swap_merge(
        &f.user, &f.token_a, &f.token_b, &1i128, &0i128, &2_000_000u64, &stages,
    );
}

/// The pre-existing balance must still be there after the attempt fails.
#[test]
fn merge_sweep_attempt_leaves_balance_untouched() {
    let f = setup(1, 1, 0);
    let token_v = env_token(&f.env);
    let contract_addr = f.client.address.clone();
    token::StellarAssetClient::new(&f.env, &token_v).mint(&contract_addr, &500_000i128);

    let stages = vec![
        &f.env,
        Stage {
            token: token_v.clone(),
            fills: vec![&f.env, fill(&f.env, &f.venue, &f.pool, &token_v, &f.token_b, 1)],
        },
    ];

    let res = f.client.try_swap_merge(
        &f.user, &f.token_a, &f.token_b, &1i128, &0i128, &2_000_000u64, &stages,
    );
    assert!(res.is_err(), "sweeping a pre-existing balance must fail");
    assert_eq!(
        token::Client::new(&f.env, &token_v).balance(&contract_addr),
        500_000,
        "the pre-existing balance must be untouched"
    );
}

/// A stage that produces a token nobody consumes is a malformed plan.
#[test]
#[should_panic(expected = "plan leaves an unconsumed balance")]
fn merge_rejects_dead_end_stage() {
    let f = setup(1, 1, 0);
    let token_c = env_token(&f.env);
    let token_d = env_token(&f.env);
    let stages = vec![
        &f.env,
        Stage {
            token: f.token_a.clone(),
            fills: vec![
                &f.env,
                fill(&f.env, &f.venue, &f.pool, &f.token_a, &token_c, 1),
                // half of the input ends up in D, which no stage consumes
                fill(&f.env, &f.venue, &f.pool, &f.token_a, &token_d, 1),
            ],
        },
        Stage {
            token: token_c.clone(),
            fills: vec![&f.env, fill(&f.env, &f.venue, &f.pool, &token_c, &f.token_b, 1)],
        },
    ];

    f.client.swap_merge(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &2_000_000u64, &stages,
    );
}

/// M-01 regression: a venue that delivers output without taking the input
/// must not be able to leave the user's funds in the contract.
#[test]
#[should_panic(expected = "venue did not consume the exact input")]
fn venue_that_does_not_pull_input_reverts() {
    let f = setup(1, 1, 0);
    MockVenueClient::new(&f.env, &f.venue).set_mode(&1u32);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
}

/// Same for partial consumption.
#[test]
#[should_panic(expected = "venue did not consume the exact input")]
fn venue_that_pulls_partially_reverts() {
    let f = setup(1, 1, 0);
    MockVenueClient::new(&f.env, &f.venue).set_mode(&2u32);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
}

/// After a successful swap the contract must hold nothing of either token.
#[test]
fn no_funds_retained_after_swap() {
    let f = setup(2, 1, 0);
    let contract_addr = f.client.address.clone();
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];
    f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &2_000_000u64, &plan,
    );
    assert_eq!(token::Client::new(&f.env, &f.token_a).balance(&contract_addr), 0);
    assert_eq!(token::Client::new(&f.env, &f.token_b).balance(&contract_addr), 0);
}

/// M-01 regression: a venue that pushes extra final token_out during an
/// earlier stage must not have that output locked in the contract. The
/// user is entitled to everything the call gained.
#[test]
fn merge_forwards_all_final_token_out_gained_this_call() {
    let f = setup(1, 1, 0);
    let token_c = env_token(&f.env);
    let contract_addr = f.client.address.clone();

    // A second venue that side-transfers 7 C while fulfilling A -> B.
    let side_pool = Address::generate(&f.env);
    let side_venue = f.env.register(MockVenue, ());
    MockVenueClient::new(&f.env, &side_venue).init(&side_pool, &1i128, &1i128, &0i128);
    token::StellarAssetClient::new(&f.env, &token_c).mint(&side_venue, &7i128);
    MockVenueClient::new(&f.env, &side_venue).set_side_output(&f.token_b, &token_c, &7i128);

    let stages = vec![
        &f.env,
        Stage {
            token: f.token_a.clone(),
            fills: vec![
                &f.env,
                fill(&f.env, &side_venue, &side_pool, &f.token_a, &f.token_b, 1),
            ],
        },
        Stage {
            token: f.token_b.clone(),
            fills: vec![&f.env, fill(&f.env, &f.venue, &f.pool, &f.token_b, &token_c, 1)],
        },
    ];

    let out = f.client.swap_merge(
        &f.user, &f.token_a, &token_c, &1_000i128, &1_000i128, &2_000_000u64, &stages,
    );

    assert_eq!(out, 1_007, "the side-delivered output belongs to the caller");
    assert_eq!(token::Client::new(&f.env, &token_c).balance(&f.user), 1_007);
    assert_eq!(
        token::Client::new(&f.env, &token_c).balance(&contract_addr),
        0,
        "nothing may be left stranded in an immutable contract"
    );
}

/// F-8 regression: `deadline == 0` means "no deadline" to this contract,
/// but a venue that compares the value against the ledger timestamp would
/// read a bare zero as a 1970 deadline. It must reach the venue as an
/// effectively unlimited value, not as zero.
#[test]
fn zero_deadline_does_not_reach_the_venue_as_zero() {
    let f = setup(1, 1, 0);
    let plan = vec![
        &f.env,
        strand(1, vec![&f.env, hop(&f.env, &f.venue, &f.pool, &f.token_a, &f.token_b)]),
    ];
    // The mock records the deadline it was handed.
    let out = f.client.swap(
        &f.user, &f.token_a, &f.token_b, &1_000i128, &1i128, &0u64, &plan,
    );
    assert_eq!(out, 1_000);
    assert_eq!(
        MockVenueClient::new(&f.env, &f.venue).last_deadline(),
        u64::MAX,
        "zero must be translated for the venue, not forwarded verbatim"
    );
}

fn env_token(env: &Env) -> Address {
    let admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(admin).address()
}
