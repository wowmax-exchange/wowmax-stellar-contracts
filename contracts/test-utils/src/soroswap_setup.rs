//! Real-Soroswap fixture: a live factory, real pair contracts and a real
//! router, deployed from the upstream wasm in
//! `adapters/soroswap/soroswap_contracts/`, with liquidity seeded into two
//! pools.
//!
//! This exists so the executor can be tested against genuine AMM contracts
//! rather than only against the in-repo mock venue. The mock stays: it can
//! misbehave on purpose (over-report, under-consume, side-transfer), which a
//! real pool never does, and those cases are what prove the executor's
//! hostile-venue guards. This fixture proves the complementary half — that
//! the executor drives an actual venue correctly.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

pub mod token {
    soroban_sdk::contractimport!(
        file = "../adapters/soroswap/soroswap_contracts/soroban_token_contract.wasm"
    );
    pub type TokenClient<'a> = Client<'a>;
}
pub use token::TokenClient;

pub fn create_token_contract<'a>(e: &Env, admin: &Address) -> TokenClient<'a> {
    TokenClient::new(
        e,
        &e.register_stellar_asset_contract_v2(admin.clone()).address(),
    )
}

pub mod pair {
    soroban_sdk::contractimport!(
        file = "../adapters/soroswap/soroswap_contracts/soroswap_pair.wasm"
    );
}

fn pair_contract_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../adapters/soroswap/soroswap_contracts/soroswap_pair.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

pub mod factory {
    soroban_sdk::contractimport!(
        file = "../adapters/soroswap/soroswap_contracts/soroswap_factory.wasm"
    );
    pub type SoroswapFactoryClient<'a> = Client<'a>;
}
pub use factory::SoroswapFactoryClient;

fn create_soroswap_factory<'a>(e: &Env, setter: &Address) -> SoroswapFactoryClient<'a> {
    let pair_hash = pair_contract_wasm(e);
    let factory_address = &e.register(factory::WASM, ());
    let factory = SoroswapFactoryClient::new(e, factory_address);
    factory.initialize(setter, &pair_hash);
    factory
}

pub mod router {
    soroban_sdk::contractimport!(
        file = "../adapters/soroswap/soroswap_contracts/soroswap_router.wasm"
    );
    pub type SoroswapRouterClient<'a> = Client<'a>;
}
pub use router::SoroswapRouterClient;

fn create_soroswap_router<'a>(e: &Env) -> SoroswapRouterClient<'a> {
    let router_address = &e.register(router::WASM, ());
    SoroswapRouterClient::new(e, router_address)
}

pub struct SoroswapTest<'a> {
    pub env: Env,
    pub router_contract: SoroswapRouterClient<'a>,
    pub factory_contract: SoroswapFactoryClient<'a>,
    pub token_0: TokenClient<'a>,
    pub token_1: TokenClient<'a>,
    pub token_2: TokenClient<'a>,
    pub user: Address,
    /// Ledger timestamp the fixture pins, so callers can pick a deadline
    /// that is in the future without guessing.
    pub ledger_timestamp: u64,
}

impl<'a> SoroswapTest<'a> {
    /// Two pools sharing token_1, so both a single hop and a two-hop route
    /// are available: token_0 <-> token_1 <-> token_2.
    pub fn soroswap_setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let router_contract = create_soroswap_router(&env);

        let initial_user_balance: i128 = 20_000_000_000_000_000_000;

        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        assert_ne!(admin, user);

        let token_0 = create_token_contract(&env, &admin);
        let token_1 = create_token_contract(&env, &admin);
        let token_2 = create_token_contract(&env, &admin);

        token_0.mint(&user, &initial_user_balance);
        token_1.mint(&user, &initial_user_balance);
        token_2.mint(&user, &initial_user_balance);

        let factory_contract = create_soroswap_factory(&env, &admin);
        env.cost_estimate().budget().reset_unlimited();

        let ledger_timestamp: u64 = 100;
        let liquidity_deadline: u64 = 1000;
        env.ledger().with_mut(|li| {
            li.timestamp = ledger_timestamp;
        });

        let amount_0: i128 = 1_000_000_000_000_000_000;
        let amount_1: i128 = 4_000_000_000_000_000_000;
        let amount_2: i128 = 8_000_000_000_000_000_000;

        router_contract.initialize(&factory_contract.address);

        router_contract.add_liquidity(
            &token_0.address,
            &token_1.address,
            &amount_0,
            &amount_1,
            &0,
            &0,
            &user,
            &liquidity_deadline,
        );

        router_contract.add_liquidity(
            &token_1.address,
            &token_2.address,
            &amount_1,
            &amount_2,
            &0,
            &0,
            &user,
            &liquidity_deadline,
        );

        SoroswapTest {
            env,
            router_contract,
            factory_contract,
            token_0,
            token_1,
            token_2,
            user,
            ledger_timestamp,
        }
    }

    /// Address the Soroswap router pulls `token_in` into for this pair —
    /// exactly what the executor pre-authorizes, so a plan must name it.
    pub fn pair(&self, a: &Address, b: &Address) -> Address {
        self.factory_contract.get_pair(a, b)
    }
}
