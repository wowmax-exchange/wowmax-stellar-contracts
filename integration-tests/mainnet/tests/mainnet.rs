//! Runs the current WOWMAX aggregator WASM against captured mainnet
//! Soroswap, Phoenix and Aquarius pools. See `EXAMPLE.md` for what each test shows.

// The generated client mirrors entry points that take seven arguments.
#![allow(clippy::too_many_arguments)]

use kanatoko::soroban_sdk::{self, contractclient, contracttype, token::TokenClient, vec};
use kanatoko::soroban_sdk::{Address, BytesN, Error, Map, Symbol, Vec};
use kanatoko::{mainnet, AutoRunner, ScenarioFork};

// Two entry points and their plan types, mirrored from the deployed interface.
#[contractclient(name = "AggregatorClient")]
pub trait Aggregator {
    fn swap(
        user: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        deadline: u64,
        plan: Vec<Strand>,
    ) -> i128;
    fn swap_merge(
        user: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        deadline: u64,
        stages: Vec<Stage>,
    ) -> i128;
}

#[contracttype]
#[derive(Clone)]
pub struct Hop {
    pub venue: u32,
    pub pool: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub aqua_router: Address,
    pub aqua_pool_tokens: Vec<Address>,
    pub aqua_pool_index: BytesN<32>,
    pub soroswap_router: Address,
    pub soroswap_path: Vec<Address>,
}

#[contracttype]
pub struct Strand {
    pub parts: u32,
    pub hops: Vec<Hop>,
}

#[contracttype]
pub struct Fill {
    pub venue: u32,
    pub pool: Address,
    pub token_out: Address,
    pub parts: u32,
    pub aqua_router: Address,
    pub aqua_pool_tokens: Vec<Address>,
    pub aqua_pool_index: BytesN<32>,
    pub soroswap_router: Address,
    pub soroswap_path: Vec<Address>,
}

#[contracttype]
pub struct Stage {
    pub token: Address,
    pub fills: Vec<Fill>,
}

const AGGREGATOR: &str = "CBMPYAEOGQUJ3LVMFXPN3X4GVNPPEI6FVG6YC7HYBYSN26KODOLUSNPF";
const XLM: &str = "CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA";
const USDC: &str = "CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75";
const AQUA: &str = "CAUIKL3IYGMERDRUN6YSCLWVAKIFG5Q4YJHUKM4S4NJZQIA3BAS6OJPK";
const SOROSWAP_ROUTER: &str = "CAG5LRYQ5JVEUI5TEID72EYOVX44TTUJT5BQR2J6J77FH65PCCFAJDDH";
const AQUA_ROUTER: &str = "CBQDHNBFBZYE4MKPWBSJOPIYLW4SFSXAXUTSXJN76GNKYVYPCKWC6QUK";
/// Index of both Aquarius pools used here, from the router's `get_pools`.
const AQUA_INDEX: [u8; 32] = [
    0x9a, 0xc7, 0xa9, 0xcd, 0xe2, 0x3a, 0xc2, 0xad, 0xa1, 0x11, 0x05, 0xee, 0xaa, 0x42, 0xe4, 0x3c,
    0x2e, 0xa8, 0x33, 0x2c, 0xa0, 0xaa, 0x8f, 0x41, 0xf5, 0x8d, 0x71, 0x60, 0x27, 0x4d, 0x71, 0x8e,
];
/// `RouterDeadlineExpired` in the error enum of the captured Soroswap router.
const ROUTER_DEADLINE_EXPIRED: u32 = 503;
/// 300 XLM: three branches of exactly 100 XLM.
const AMOUNT: i128 = 3_000_000_000;

/// Replay the committed pool state for this scenario.
fn runner(bundle: &str) -> AutoRunner {
    mainnet()
        .cache(format!("{}/fixtures/{bundle}", env!("CARGO_MANIFEST_DIR")))
        .offline()
}

/// The aggregator, and a local account holding XLM with a USDC trustline.
/// No real account or key is involved.
fn setup<'a>(fork: &ScenarioFork<'a>) -> (AggregatorClient<'a>, Address) {
    fork.mock_all_auths();
    fork.replace_wasm(
        &fork.contract(AGGREGATOR),
        include_bytes!("../target/wasm32v1-none/release/wowmax_stellar_router.wasm"),
    );
    let user = fork.local_account("trader");
    fork.fund_local_account(&user, 10 * AMOUNT as i64);
    fork.invoke::<()>(&fork.contract(USDC), "trust", (user.clone(),));
    (
        AggregatorClient::new(fork.env(), &fork.contract(AGGREGATOR)),
        user,
    )
}

/// One edge; venue 0 is Soroswap, 1 Aquarius, 2 Phoenix. The aggregator reads
/// only the fields of the edge's venue. Each edge here runs from the lower token
/// address to the higher, the order Aquarius expects for its pool tokens.
fn hop(fork: &ScenarioFork<'_>, venue: u32, pool: &str, token_in: &str, token_out: &str) -> Hop {
    let path = vec![
        fork.env(),
        fork.contract(token_in),
        fork.contract(token_out),
    ];
    Hop {
        venue,
        pool: fork.contract(pool),
        token_in: path.get_unchecked(0),
        token_out: path.get_unchecked(1),
        aqua_router: fork.contract(AQUA_ROUTER),
        aqua_pool_tokens: path.clone(),
        aqua_pool_index: BytesN::from_array(fork.env(), &AQUA_INDEX),
        soroswap_router: fork.contract(SOROSWAP_ROUTER),
        soroswap_path: path,
    }
}

/// The four real pools the plans here route through.
fn hops(fork: &ScenarioFork<'_>) -> [Hop; 4] {
    [
        hop(
            fork,
            0,
            "CAM7DY53G63XA4AJRS24Z6VFYAFSSF76C3RZ45BE5YU3FQS5255OOABP",
            XLM,
            USDC,
        ),
        hop(
            fork,
            2,
            "CBHCRSVX3ZZ7EGTSYMKPEFGZNWRVCSESQR3UABET4MIW52N4EVU6BIZX",
            XLM,
            USDC,
        ),
        hop(
            fork,
            1,
            "CCY2PXGMKNQHO7WNYXEWX76L2C5BH3JUW3RCATGUYKY7QQTRILBZIFWV",
            XLM,
            AQUA,
        ),
        hop(
            fork,
            1,
            "CA6GAFOJCW4MGQQBUCQUSA3CLIH25G4SNKB2JHYKZCVWZTNW5VXMSC4O",
            AQUA,
            USDC,
        ),
    ]
}

/// What a pool itself quotes for an edge, through its own read-only method.
fn quote(fork: &ScenarioFork<'_>, hop: &Hop, amount: i128) -> i128 {
    if hop.venue == 0 {
        let path = (amount, &hop.soroswap_path);
        let amounts: Vec<i128> = fork.invoke(&hop.soroswap_router, "router_get_amounts_out", path);
        amounts.get_unchecked(1)
    } else if hop.venue == 1 {
        fork.invoke::<u128>(&hop.pool, "estimate_swap", (0_u32, 1_u32, amount as u128)) as i128
    } else {
        let sim: Map<Symbol, i128> =
            fork.invoke(&hop.pool, "simulate_swap", (&hop.token_in, amount));
        sim.get_unchecked(Symbol::new(fork.env(), "ask_amount"))
    }
}

/// The quoted USDC for 100 XLM through each of Soroswap, Phoenix and Aquarius via AQUA.
fn quoted(fork: &ScenarioFork<'_>, [soroswap, phoenix, to_aqua, from_aqua]: &[Hop; 4]) -> i128 {
    let leg = AMOUNT / 3;
    let aquarius = quote(fork, from_aqua, quote(fork, to_aqua, leg));
    quote(fork, soroswap, leg) + quote(fork, phoenix, leg) + aquarius
}

/// A `swap` strand of one part running `hops` in sequence.
fn strand(fork: &ScenarioFork<'_>, hops: &[Hop]) -> Strand {
    Strand {
        parts: 1,
        hops: Vec::from_slice(fork.env(), hops),
    }
}

/// A `swap_merge` stage splitting its token across `hops`, one part each.
fn stage(fork: &ScenarioFork<'_>, hops: &[Hop]) -> Stage {
    let mut fills = Vec::new(fork.env());
    for hop in hops.iter().cloned() {
        fills.push_back(Fill {
            venue: hop.venue,
            pool: hop.pool,
            token_out: hop.token_out,
            parts: 1,
            aqua_router: hop.aqua_router,
            aqua_pool_tokens: hop.aqua_pool_tokens,
            aqua_pool_index: hop.aqua_pool_index,
            soroswap_router: hop.soroswap_router,
            soroswap_path: hop.soroswap_path,
        });
    }
    Stage {
        token: hops[0].token_in.clone(),
        fills,
    }
}

#[test]
fn split_pays_exactly_the_pool_quotes_and_the_minimum_is_inclusive() {
    runner("split.capture.json")
        .run(|fork| {
            let (aggregator, user) = setup(fork);
            let (env, xlm, usdc) = (fork.env(), fork.contract(XLM), fork.contract(USDC));
            let hops = hops(fork);
            let quoted = quoted(fork, &hops);
            let [soroswap, phoenix, to_aqua, from_aqua] = hops;
            let plan = vec![
                env,
                strand(fork, &[soroswap]),
                strand(fork, &[phoenix]),
                strand(fork, &[to_aqua, from_aqua]),
            ];
            let deadline = env.ledger().timestamp() + 900;

            // One stroop above the quotes reverts, and the failed call changes nothing.
            let above = quoted + 1;
            let missed = aggregator.try_swap(&user, &xlm, &usdc, &AMOUNT, &above, &deadline, &plan);
            assert!(
                missed.is_err(),
                "the aggregator rejects it: amount_out_min not met"
            );

            let out = aggregator.swap(&user, &xlm, &usdc, &AMOUNT, &quoted, &deadline, &plan);
            assert_eq!(out, quoted);
            assert_eq!(TokenClient::new(env, &usdc).balance(&user), quoted);
        })
        .expect("the test must replay from its bundle");
}

#[test]
fn merge_of_the_same_pools_pays_the_same_with_no_deadline() {
    runner("merge.capture.json")
        .run(|fork| {
            let (aggregator, user) = setup(fork);
            let (env, xlm, usdc) = (fork.env(), fork.contract(XLM), fork.contract(USDC));
            let hops = hops(fork);
            let quoted = quoted(fork, &hops);
            let [soroswap, phoenix, to_aqua, from_aqua] = hops;
            let stages = vec![
                env,
                stage(fork, &[soroswap, phoenix, to_aqua]),
                stage(fork, &[from_aqua]),
            ];

            // A zero deadline means no deadline.
            let out = aggregator.swap_merge(&user, &xlm, &usdc, &AMOUNT, &quoted, &0, &stages);
            assert_eq!(out, quoted);
            assert_eq!(TokenClient::new(env, &usdc).balance(&user), quoted);
            for token in [xlm, usdc, fork.contract(AQUA)] {
                assert_eq!(
                    TokenClient::new(env, &token).balance(&aggregator.address),
                    0
                );
            }
        })
        .expect("the test must replay from its bundle");
}

#[test]
fn deadline_equal_to_now_passes_phoenix_and_aquarius_but_not_soroswap() {
    runner("deadline.capture.json")
        .run(|fork| {
            let (aggregator, user) = setup(fork);
            let (env, xlm, usdc) = (fork.env(), fork.contract(XLM), fork.contract(USDC));
            let [soroswap, phoenix, to_aqua, from_aqua] = hops(fork);
            let now = env.ledger().timestamp();

            let plan = vec![env, strand(fork, &[soroswap])];
            let refused = aggregator.try_swap(&user, &xlm, &usdc, &AMOUNT, &0, &now, &plan);
            assert_eq!(
                refused,
                Err(Ok(Error::from_contract_error(ROUTER_DEADLINE_EXPIRED)))
            );

            let plan = vec![
                env,
                strand(fork, &[phoenix]),
                strand(fork, &[to_aqua, from_aqua]),
            ];
            let out = aggregator.swap(&user, &xlm, &usdc, &AMOUNT, &1, &now, &plan);
            assert_eq!(TokenClient::new(env, &usdc).balance(&user), out);
        })
        .expect("the test must replay from its bundle");
}
