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

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{
    contract, contractimpl, symbol_short, token, vec, Address, BytesN, Env, Vec,
};

use crate::{Hop, Strand, WowmaxAggregator, WowmaxAggregatorClient};

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
    }

    pub fn swap_exact_tokens_for_tokens(
        env: Env,
        amount_in: i128,
        _amount_out_min: i128,
        path: Vec<Address>,
        to: Address,
        _deadline: u64,
    ) -> Vec<i128> {
        let pool: Address = env.storage().instance().get(&symbol_short!("pool")).unwrap();
        let num: i128 = env.storage().instance().get(&symbol_short!("num")).unwrap();
        let den: i128 = env.storage().instance().get(&symbol_short!("den")).unwrap();
        let bonus: i128 = env.storage().instance().get(&symbol_short!("bonus")).unwrap();

        let token_in = path.get(0).unwrap();
        let token_out = path.last().unwrap();

        // Pull the input exactly as the real router does.
        token::Client::new(&env, &token_in).transfer(&to, &pool, &amount_in);

        let delivered = amount_in * num / den;
        token::StellarAssetClient::new(&env, &token_out).mint(&to, &delivered);

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

fn env_token(env: &Env) -> Address {
    let admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(admin).address()
}
