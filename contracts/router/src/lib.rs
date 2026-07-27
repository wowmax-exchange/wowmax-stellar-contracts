#![no_std]
//! WOWMAX Stellar aggregator — on-chain executor of VFalgo routes.
//!
//! Thin plan executor. All routing intelligence (VFalgo) stays
//! OFF-chain; the contract only executes what it is handed. No VFalgo
//! IP lives here.
//!
//! Progress:
//!   S1  swap_soroswap  — one Soroswap path swap (DONE, mainnet)
//!   S2  swap_aqua      — one Aquarius swap_chained hop (this file)
//! Next: cross-protocol single-call plan, then the parts splitter (S4).

use soroban_sdk::auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation};
use soroban_sdk::Map;
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, vec, Address, BytesN, Env,
    IntoVal, Symbol, Val, Vec,
};

/// One edge of a route: a single swap on one venue. Flat struct (all
/// fields present); the off-chain planner fills the venue-specific
/// fields and leaves the rest as harmless placeholders (empty Vec, zero
/// BytesN, any valid Address) for venues that don't use them.
///   venue: 0 = Soroswap, 1 = Aquarius, 2 = Phoenix
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

/// One parallel branch of the split. `parts` is the integer share of the
/// total input (same model as the router's buildSorobanDistribution:
/// strand_in = floor(amount_in * parts / total_parts), last strand takes
/// the remainder). `hops` runs sequentially (multi-hop) within the branch.
#[contracttype]
#[derive(Clone)]
pub struct Strand {
    pub parts: u32,
    pub hops: Vec<Hop>,
}

#[contracttype]
#[derive(Clone)]
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
#[derive(Clone)]
pub struct Stage {
    pub token: Address,
    pub fills: Vec<Fill>,
}

#[cfg(test)]
mod test;

/// Reject non-positive input and negative minimums before any cast or
/// transfer. `amount_in as u128` on a negative i128 wraps to a huge u128
/// (an `as` cast is not covered by overflow-checks), so this guard is the
/// only thing standing between a malformed plan and the Aquarius call.
fn require_amounts(amount_in: i128, amount_out_min: i128) {
    if amount_in <= 0 {
        panic!("amount_in must be positive");
    }
    if amount_out_min < 0 {
        panic!("amount_out_min must be non-negative");
    }
}

/// Enforce the deadline on-chain instead of relying on each venue to do
/// it (Phoenix is called with deadline = None, so without this the value
/// was decorative on that venue). `deadline == 0` means "no deadline".
fn require_deadline(env: &Env, deadline: u64) {
    if deadline != 0 && env.ledger().timestamp() > deadline {
        panic!("deadline passed");
    }
}

/// The end-to-end input and output token must differ: every accounting
/// path below measures the contract's token_out balance delta, and a
/// self-swap would fold the pulled input into that delta.
/// A venue may pull at most the amount the contract pre-authorized, but
/// nothing forces it to pull anything at all. If it under-consumes, the
/// user's input silently stays in the contract. Requiring exact
/// consumption turns that into a revert instead of a stranded balance.
fn require_consumed(env: &Env, token: &Address, contract: &Address, before: i128, expected: i128) {
    let after: i128 = token::Client::new(env, token).balance(contract);
    if before - after != expected {
        panic!("venue did not consume the exact input");
    }
}

fn require_distinct(token_in: &Address, token_out: &Address) {
    if token_in == token_out {
        panic!("token_in equals token_out");
    }
}

#[contract]
pub struct WowmaxAggregator;

#[contractimpl]
impl WowmaxAggregator {
    /// One Soroswap swap along `path`. (S1 — proven on mainnet.)
    pub fn swap_soroswap(
        env: Env,
        user: Address,
        soroswap_router: Address,
        pool: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        path: Vec<Address>,
        deadline: u64,
    ) -> i128 {
        user.require_auth();
        require_amounts(amount_in, amount_out_min);
        require_distinct(&token_in, &token_out);
        require_deadline(&env, deadline);
        let contract = env.current_contract_address();

        let out_before: i128 = token::Client::new(&env, &token_out).balance(&contract);

        token::Client::new(&env, &token_in).transfer(&user, &contract, &amount_in);
        let src_before: i128 = token::Client::new(&env, &token_in).balance(&contract);

        let transfer_args: Vec<Val> = vec![
            &env,
            contract.into_val(&env),
            pool.into_val(&env),
            amount_in.into_val(&env),
        ];
        env.authorize_as_current_contract(vec![
            &env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token_in.clone(),
                    fn_name: symbol_short!("transfer"),
                    args: transfer_args,
                },
                sub_invocations: vec![&env],
            }),
        ]);

        let args: Vec<Val> = vec![
            &env,
            amount_in.into_val(&env),
            amount_out_min.into_val(&env),
            path.into_val(&env),
            contract.into_val(&env),
            deadline.into_val(&env),
        ];
        let amounts: Vec<i128> = env.invoke_contract(
            &soroswap_router,
            &Symbol::new(&env, "swap_exact_tokens_for_tokens"),
            args,
        );
        if amounts.is_empty() {
            panic!("empty router response");
        }
        require_consumed(&env, &token_in, &contract, src_before, amount_in);
        // Trust the balance we actually hold, not the number the venue
        // reported: an inflated return would otherwise make the forward
        // transfer exceed the contract's balance.
        let out_after: i128 = token::Client::new(&env, &token_out).balance(&contract);
        let out: i128 = out_after - out_before;
        if out <= 0 {
            panic!("no output");
        }
        if out < amount_out_min {
            panic!("amount_out_min not met");
        }
        token::Client::new(&env, &token_out).transfer(&contract, &user, &out);
        out
    }

    /// One Aquarius swap through the router's `swap_chained`.
    ///
    /// `pool_tokens`  — the pool's ordered token vector (canonical, by
    ///                  contract-id). For USDC/AQUA: [AQUA_SAC, USDC_SAC].
    /// `pool_index`   — the pool hash (BytesN<32>) from get_pools.
    /// `pool`         — the pool contract address (the router pulls
    ///                  token_in into it; used for the auth subtree).
    /// `token_in/out` — SAC contract ids.
    ///
    /// swap_chained(user, swaps_chain, token_in, in_amount, out_min):
    ///   swaps_chain = [ (pool_tokens, pool_index, token_out) ]   (single hop)
    ///
    /// Returns the amount of token_out delivered (u128 -> i128).
    pub fn swap_aqua(
        env: Env,
        user: Address,
        aqua_router: Address,
        pool: Address,
        pool_tokens: Vec<Address>,
        pool_index: BytesN<32>,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        deadline: u64,
    ) -> i128 {
        user.require_auth();
        require_amounts(amount_in, amount_out_min);
        require_distinct(&token_in, &token_out);
        require_deadline(&env, deadline);
        let contract = env.current_contract_address();
        let _ = &pool; // retained in signature for call-site compatibility; Aquarius auth targets the router, not the pool

        let out_before: i128 = token::Client::new(&env, &token_out).balance(&contract);

        // 1) Pull input from the user.
        token::Client::new(&env, &token_in).transfer(&user, &contract, &amount_in);

        let src_before: i128 = token::Client::new(&env, &token_in).balance(&contract);

        // 2) Pre-authorize the router to move our token_in. Aquarius's
        //    swap_chained pulls token_in from the holder TO THE ROUTER
        //    itself (confirmed by simulation: transfer [contract ->
        //    aqua_router]), then the router fans out to its pools. So the
        //    authorized transfer target is `aqua_router`, NOT the pool.
        let transfer_args: Vec<Val> = vec![
            &env,
            contract.into_val(&env),
            aqua_router.into_val(&env),
            amount_in.into_val(&env),
        ];
        env.authorize_as_current_contract(vec![
            &env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token_in.clone(),
                    fn_name: symbol_short!("transfer"),
                    args: transfer_args,
                },
                sub_invocations: vec![&env],
            }),
        ]);

        // 3) Build swaps_chain = Vec<(Vec<Address>, BytesN<32>, Address)>
        //    with a single hop, as an ScVal vector of 3-tuples (scvVec).
        let hop: Vec<Val> = vec![
            &env,
            pool_tokens.into_val(&env),
            pool_index.into_val(&env),
            token_out.into_val(&env),
        ];
        let swaps_chain: Vec<Val> = vec![&env, hop.into_val(&env)];

        let amount_in_u128: u128 = amount_in as u128;
        let out_min_u128: u128 = amount_out_min as u128;

        let args: Vec<Val> = vec![
            &env,
            // swap_chained pulls token_in FROM and delivers token_out TO
            // this first arg. The contract holds the funds, so it is the
            // contract — NOT the end user (who no longer holds token_in).
            contract.into_val(&env),
            swaps_chain.into_val(&env),
            token_in.into_val(&env),
            amount_in_u128.into_val(&env),
            out_min_u128.into_val(&env),
        ];
        let _reported: u128 = env.invoke_contract(
            &aqua_router,
            &Symbol::new(&env, "swap_chained"),
            args,
        );
        require_consumed(&env, &token_in, &contract, src_before, amount_in);
        // Balance delta, not the reported figure (see swap_soroswap).
        let out_after: i128 = token::Client::new(&env, &token_out).balance(&contract);
        let out: i128 = out_after - out_before;
        if out <= 0 {
            panic!("no output");
        }

        // 4) Slippage guard.
        if out < amount_out_min {
            panic!("amount_out_min not met");
        }

        // 5) Forward proceeds to the user.
        token::Client::new(&env, &token_out).transfer(&contract, &user, &out);
        out
    }

    /// One Phoenix swap. Phoenix trades through the POOL contract directly
    /// (no router), unlike Soroswap/Aquarius.
    ///
    ///   pool.swap(sender, offer_asset, offer_amount,
    ///             max_belief_price: Option<i64>, max_spread_bps: Option<i64>)
    ///       -> i128 (amount of the other asset received)
    ///
    /// `sender` is the CONTRACT (it holds the funds and receives output).
    /// We pass None/None for price & spread limits; the final slippage
    /// guard enforces amount_out_min end-to-end.
    pub fn swap_phoenix(
        env: Env,
        user: Address,
        pool: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        deadline: u64,
    ) -> i128 {
        user.require_auth();
        require_amounts(amount_in, amount_out_min);
        require_distinct(&token_in, &token_out);
        require_deadline(&env, deadline);
        let contract = env.current_contract_address();

        let out_before: i128 = token::Client::new(&env, &token_out).balance(&contract);

        // 1) Pull input from the user.
        token::Client::new(&env, &token_in).transfer(&user, &contract, &amount_in);

        let src_before: i128 = token::Client::new(&env, &token_in).balance(&contract);

        // 2) Pre-authorize the pool to move our token_in. Target guess =
        //    the pool itself (Phoenix pools pull the offer). If simulation
        //    shows a different target, swap `pool` for it (as with Aqua).
        let transfer_args: Vec<Val> = vec![
            &env,
            contract.into_val(&env),
            pool.into_val(&env),
            amount_in.into_val(&env),
        ];
        env.authorize_as_current_contract(vec![
            &env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token_in.clone(),
                    fn_name: symbol_short!("transfer"),
                    args: transfer_args,
                },
                sub_invocations: vec![&env],
            }),
        ]);

        // 3) pool.swap has 7 params (verified from on-chain spec):
        //    swap(sender, offer_asset, offer_amount,
        //         ask_asset_min_amount: Option<i128>,
        //         max_spread_bps:       Option<i64>,
        //         deadline:             Option<u64>,
        //         max_allowed_fee_bps:  Option<i64>) -> i128
        //    Option::Some(x) is passed as x itself; Option::None as Void.
        //    Enforce slippage in-pool via ask_asset_min_amount = out_min.
        let none_val: Val = ().into_val(&env);
        let args: Vec<Val> = vec![
            &env,
            contract.into_val(&env),
            token_in.into_val(&env),
            amount_in.into_val(&env),
            amount_out_min.into_val(&env), // ask_asset_min_amount = Some(out_min)
            none_val,                      // max_spread_bps = None
            if deadline == 0 {
                none_val
            } else {
                Some(deadline).into_val(&env)
            }, // deadline
            none_val,                      // max_allowed_fee_bps = None
        ];
        let _reported: i128 = env.invoke_contract(&pool, &Symbol::new(&env, "swap"), args);
        require_consumed(&env, &token_in, &contract, src_before, amount_in);
        // Balance delta, not the reported figure (see swap_soroswap).
        let out_after: i128 = token::Client::new(&env, &token_out).balance(&contract);
        let out: i128 = out_after - out_before;
        if out <= 0 {
            panic!("no output");
        }

        // 4) Slippage guard.
        if out < amount_out_min {
            panic!("amount_out_min not met");
        }

        // 5) Forward proceeds to the user.
        token::Client::new(&env, &token_out).transfer(&contract, &user, &out);
        out
    }

    /// CROSS-PROTOCOL chain in ONE call (S3): leg 1 Aquarius, leg 2
    /// Soroswap.  token_in --[aqua]--> mid_token --[soroswap]--> token_out.
    ///
    /// The contract holds the intermediate (mid_token) and feeds its
    /// ACTUAL balance into leg 2 — the exact mechanic the parts splitter
    /// (S4) generalizes. Soroswap's own aggregator cannot do this: one
    /// DexDistribution path is single-protocol.
    ///
    /// Auth targets are the ones proven on mainnet:
    ///   - Aquarius pulls token_in to the ROUTER  (swap_aqua / S2)
    ///   - Soroswap pulls mid_token to the POOL    (swap_soroswap / S1)
    pub fn swap_aqua_then_soroswap(
        env: Env,
        user: Address,
        // leg 1 (Aquarius): token_in -> mid_token
        aqua_router: Address,
        aqua_pool_tokens: Vec<Address>,
        aqua_pool_index: BytesN<32>,
        token_in: Address,
        mid_token: Address,
        // leg 2 (Soroswap): mid_token -> token_out
        soroswap_router: Address,
        soroswap_pool: Address,
        soroswap_path: Vec<Address>,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        deadline: u64,
    ) -> i128 {
        user.require_auth();
        require_amounts(amount_in, amount_out_min);
        require_distinct(&token_in, &token_out);
        require_deadline(&env, deadline);
        if mid_token == token_in || mid_token == token_out {
            panic!("mid_token must differ from token_in and token_out");
        }
        let contract = env.current_contract_address();

        let out_before: i128 = token::Client::new(&env, &token_out).balance(&contract);
        // Measure the intermediate token the SAME way: only what leg 1
        // produces may fund leg 2. Reading the absolute balance would
        // sweep any pre-existing mid_token dust held by the contract.
        let mid_before: i128 = token::Client::new(&env, &mid_token).balance(&contract);

        // Pull leg-1 input from the user.
        token::Client::new(&env, &token_in).transfer(&user, &contract, &amount_in);
        let src_before: i128 = token::Client::new(&env, &token_in).balance(&contract);

        // ---- LEG 1: Aquarius swap_chained (token_in -> mid_token) ----
        let l1_transfer: Vec<Val> = vec![
            &env,
            contract.into_val(&env),
            aqua_router.into_val(&env),
            amount_in.into_val(&env),
        ];
        env.authorize_as_current_contract(vec![
            &env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token_in.clone(),
                    fn_name: symbol_short!("transfer"),
                    args: l1_transfer,
                },
                sub_invocations: vec![&env],
            }),
        ]);
        let hop: Vec<Val> = vec![
            &env,
            aqua_pool_tokens.into_val(&env),
            aqua_pool_index.into_val(&env),
            mid_token.into_val(&env),
        ];
        let swaps_chain: Vec<Val> = vec![&env, hop.into_val(&env)];
        let l1_args: Vec<Val> = vec![
            &env,
            contract.into_val(&env),
            swaps_chain.into_val(&env),
            token_in.into_val(&env),
            (amount_in as u128).into_val(&env),
            0u128.into_val(&env),
        ];
        let _mid_out: u128 = env.invoke_contract(
            &aqua_router,
            &Symbol::new(&env, "swap_chained"),
            l1_args,
        );

        require_consumed(&env, &token_in, &contract, src_before, amount_in);
        // Leg-1 delta = leg-2 input.
        let mid_after: i128 = token::Client::new(&env, &mid_token).balance(&contract);
        let mid_amt: i128 = mid_after - mid_before;
        if mid_amt <= 0 {
            panic!("leg 1 produced no output");
        }

        // ---- LEG 2: Soroswap swap_exact_tokens_for_tokens (mid -> out) ----
        let l2_transfer: Vec<Val> = vec![
            &env,
            contract.into_val(&env),
            soroswap_pool.into_val(&env),
            mid_amt.into_val(&env),
        ];
        env.authorize_as_current_contract(vec![
            &env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: mid_token.clone(),
                    fn_name: symbol_short!("transfer"),
                    args: l2_transfer,
                },
                sub_invocations: vec![&env],
            }),
        ]);
        let l2_args: Vec<Val> = vec![
            &env,
            mid_amt.into_val(&env),
            0i128.into_val(&env),
            soroswap_path.into_val(&env),
            contract.into_val(&env),
            deadline.into_val(&env),
        ];
        let amounts: Vec<i128> = env.invoke_contract(
            &soroswap_router,
            &Symbol::new(&env, "swap_exact_tokens_for_tokens"),
            l2_args,
        );
        if amounts.is_empty() {
            panic!("empty router response");
        }
        require_consumed(&env, &mid_token, &contract, mid_after, mid_amt);
        let out_after: i128 = token::Client::new(&env, &token_out).balance(&contract);
        let out: i128 = out_after - out_before;
        if out <= 0 {
            panic!("no output");
        }

        // Final slippage guard on the end-to-end output.
        if out < amount_out_min {
            panic!("amount_out_min not met");
        }

        // Forward proceeds to the user.
        token::Client::new(&env, &token_out).transfer(&contract, &user, &out);
        out
    }

    /// S4 — SPLITTER. Execute a full VFalgo plan in ONE call: parallel
    /// strands (split), each with sequential hops (multi-hop), across any
    /// mix of Soroswap / Aquarius / Phoenix. Slippage is enforced on the
    /// SUM of all strand outputs (atomic across the whole plan).
    ///
    ///   strand_in = floor(amount_in * parts / total_parts); the last
    ///   strand takes the remainder so the split sums to amount_in exactly.
    ///   Within a strand, each hop consumes the previous hop's output.
    pub fn swap(
        env: Env,
        user: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        deadline: u64,
        plan: Vec<Strand>,
    ) -> i128 {
        user.require_auth();
        require_amounts(amount_in, amount_out_min);
        require_distinct(&token_in, &token_out);
        require_deadline(&env, deadline);
        let contract = env.current_contract_address();

        let n = plan.len();
        if n == 0 {
            panic!("empty plan");
        }

        let out_before: i128 = token::Client::new(&env, &token_out).balance(&contract);

        // Pull the whole input once.
        token::Client::new(&env, &token_in).transfer(&user, &contract, &amount_in);

        // Sum parts.
        let mut total_parts: i128 = 0;
        let mut i = 0u32;
        while i < n {
            total_parts += plan.get(i).unwrap().parts as i128;
            i += 1;
        }
        if total_parts <= 0 {
            panic!("total parts zero");
        }

        // Execute strands.
        let mut allocated: i128 = 0;
        let mut s = 0u32;
        while s < n {
            let strand = plan.get(s).unwrap();
            let strand_in: i128 = if s == n - 1 {
                amount_in - allocated
            } else {
                (amount_in * (strand.parts as i128)) / total_parts
            };
            allocated += strand_in;

            // Sequential hops; each consumes the previous hop's output.
            let hops = strand.hops;
            let hn = hops.len();
            if hn == 0 {
                panic!("empty strand");
            }
            let mut hop_in: i128 = strand_in;
            let mut h = 0u32;
            while h < hn {
                let hop = hops.get(h).unwrap();

                // Token continuity. Without these three checks a plan may
                // name any token in `hop.token_in` and the contract would
                // spend whatever it happens to hold of it (another
                // strand's proceeds, or residue) instead of this hop's
                // input.
                if hop.token_in == hop.token_out {
                    panic!("degenerate hop");
                }
                if h == 0 {
                    if hop.token_in != token_in {
                        panic!("strand must start at token_in");
                    }
                } else if hop.token_in != hops.get(h - 1).unwrap().token_out {
                    panic!("hop token mismatch");
                }
                if h == hn - 1 && hop.token_out != token_out {
                    panic!("strand must end at token_out");
                }

                // Measure what the venue actually delivered AND that it
                // took exactly what it was given.
                let hop_before: i128 =
                    token::Client::new(&env, &hop.token_out).balance(&contract);
                let src_before: i128 =
                    token::Client::new(&env, &hop.token_in).balance(&contract);
                if hop.venue == 0 {
                    exec_soroswap_edge(
                        &env, &contract, &hop.soroswap_router, &hop.pool, &hop.token_in,
                        hop_in, &hop.soroswap_path, deadline,
                    );
                } else if hop.venue == 1 {
                    exec_aqua_edge(
                        &env, &contract, &hop.aqua_router, &hop.aqua_pool_tokens,
                        &hop.aqua_pool_index, &hop.token_in, &hop.token_out, hop_in,
                    );
                } else if hop.venue == 2 {
                    exec_phoenix_edge(&env, &contract, &hop.pool, &hop.token_in, hop_in, deadline);
                } else {
                    panic!("bad venue");
                }
                require_consumed(&env, &hop.token_in, &contract, src_before, hop_in);
                let hop_after: i128 =
                    token::Client::new(&env, &hop.token_out).balance(&contract);
                let out: i128 = hop_after - hop_before;
                if out <= 0 {
                    panic!("hop produced no output");
                }
                hop_in = out;
                h += 1;
            }
            s += 1;
        }

        // Atomic slippage guard on the token_out actually gained by this
        // call, summed across every strand.
        let out_after: i128 = token::Client::new(&env, &token_out).balance(&contract);
        let total_out: i128 = out_after - out_before;
        if total_out <= 0 {
            panic!("no output");
        }
        if total_out < amount_out_min {
            panic!("amount_out_min not met");
        }

        // Forward all proceeds to the user.
        token::Client::new(&env, &token_out).transfer(&contract, &user, &total_out);
        total_out
    }

    /// S5 merge executor: run a topologically-ordered DAG of stages. Each stage
    /// splits the contract's CURRENT balance of its source token across its
    /// fills (one pool swap per fill). A token consumed by several branches is
    /// split ONCE on the pooled total -> fan-in/merge, one swap per graph edge.
    pub fn swap_merge(
        env: Env,
        user: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        deadline: u64,
        stages: Vec<Stage>,
    ) -> i128 {
        user.require_auth();
        require_amounts(amount_in, amount_out_min);
        require_distinct(&token_in, &token_out);
        require_deadline(&env, deadline);
        let contract = env.current_contract_address();

        let n = stages.len();
        if n == 0 {
            panic!("empty stages");
        }

        // Net-of-dust: measure token_out gained by THIS call.
        let out_before: i128 = token::Client::new(&env, &token_out).balance(&contract);

        // Pull the whole input once.
        token::Client::new(&env, &token_in).transfer(&user, &contract, &amount_in);

        // Current-call provenance. Only tokens this call pulled in or
        // produced may be spent; the contract's pre-existing balances are
        // invisible to the plan. Without this a crafted stage list could
        // name any token the contract happens to hold and hand it to an
        // attacker-controlled "venue" through the pre-authorized transfer.
        let mut avail: Map<Address, i128> = Map::new(&env);
        avail.set(token_in.clone(), amount_in);

        let mut si = 0u32;
        while si < n {
            let stage = stages.get(si).unwrap();
            let stage_token = stage.token.clone();

            let bal: i128 = avail.get(stage_token.clone()).unwrap_or(0);
            if bal <= 0 {
                panic!("stage token has no current-call balance");
            }

            let fills = stage.fills;
            let fcount = fills.len();
            if fcount == 0 {
                panic!("empty stage");
            }

            let mut total_parts: i128 = 0;
            let mut fi = 0u32;
            while fi < fcount {
                total_parts += fills.get(fi).unwrap().parts as i128;
                fi += 1;
            }
            if total_parts <= 0 {
                panic!("stage parts zero");
            }

            let mut allocated: i128 = 0;
            fi = 0u32;
            while fi < fcount {
                let fill = fills.get(fi).unwrap();
                let fill_in: i128 = if fi == fcount - 1 {
                    bal - allocated
                } else {
                    (bal * (fill.parts as i128)) / total_parts
                };
                allocated += fill_in;

                if fill.token_out == stage_token {
                    panic!("degenerate fill");
                }
                if fill_in > 0 {
                    let src_before: i128 =
                        token::Client::new(&env, &stage_token).balance(&contract);
                    let dst_before: i128 =
                        token::Client::new(&env, &fill.token_out).balance(&contract);
                    if fill.venue == 0 {
                        exec_soroswap_edge(
                            &env, &contract, &fill.soroswap_router, &fill.pool,
                            &stage_token, fill_in, &fill.soroswap_path, deadline,
                        );
                    } else if fill.venue == 1 {
                        exec_aqua_edge(
                            &env, &contract, &fill.aqua_router, &fill.aqua_pool_tokens,
                            &fill.aqua_pool_index, &stage_token, &fill.token_out, fill_in,
                        );
                    } else if fill.venue == 2 {
                        exec_phoenix_edge(
                            &env, &contract, &fill.pool, &stage_token, fill_in, deadline,
                        );
                    } else {
                        panic!("bad venue");
                    }
                    require_consumed(&env, &stage_token, &contract, src_before, fill_in);
                    let dst_after: i128 =
                        token::Client::new(&env, &fill.token_out).balance(&contract);
                    let produced: i128 = dst_after - dst_before;
                    if produced <= 0 {
                        panic!("fill produced no output");
                    }
                    let src_left: i128 = avail.get(stage_token.clone()).unwrap_or(0) - fill_in;
                    avail.set(stage_token.clone(), src_left);
                    let dst_have: i128 = avail.get(fill.token_out.clone()).unwrap_or(0);
                    avail.set(fill.token_out.clone(), dst_have + produced);
                }
                fi += 1;
            }
            si += 1;
        }

        // Nothing this call created may be left behind: every tracked
        // token other than token_out must be fully consumed. A dead-end
        // stage is a malformed plan, not a donation to the contract.
        let keys = avail.keys();
        let kn = keys.len();
        let mut ki = 0u32;
        while ki < kn {
            let k = keys.get(ki).unwrap();
            if k != token_out && avail.get(k.clone()).unwrap_or(0) != 0 {
                panic!("plan leaves an unconsumed balance");
            }
            ki += 1;
        }

        let total_out: i128 = avail.get(token_out.clone()).unwrap_or(0);
        if total_out <= 0 {
            panic!("no output");
        }
        if total_out < amount_out_min {
            panic!("amount_out_min not met");
        }
        // Cross-check against the contract's own books before paying out.
        let out_after: i128 = token::Client::new(&env, &token_out).balance(&contract);
        if out_after - out_before < total_out {
            panic!("accounting mismatch");
        }

        token::Client::new(&env, &token_out).transfer(&contract, &user, &total_out);
        total_out
    }
}

// ----- internal per-venue edge executors (no user pull / no forward) -----
// Each authorizes the venue's token pull (target proven on mainnet:
// Soroswap -> pool, Aquarius -> router, Phoenix -> pool), invokes the
// swap, and returns the output amount actually delivered to the contract.

fn exec_soroswap_edge(
    env: &Env,
    contract: &Address,
    router: &Address,
    pool: &Address,
    token_in: &Address,
    amount_in: i128,
    path: &Vec<Address>,
    deadline: u64,
) {
    if amount_in <= 0 {
        panic!("hop amount must be positive");
    }
    let transfer_args: Vec<Val> = vec![
        env,
        contract.into_val(env),
        pool.into_val(env),
        amount_in.into_val(env),
    ];
    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: token_in.clone(),
                fn_name: symbol_short!("transfer"),
                args: transfer_args,
            },
            sub_invocations: vec![env],
        }),
    ]);
    let args: Vec<Val> = vec![
        env,
        amount_in.into_val(env),
        0i128.into_val(env),
        path.into_val(env),
        contract.into_val(env),
        deadline.into_val(env),
    ];
    let amounts: Vec<i128> =
        env.invoke_contract(router, &Symbol::new(env, "swap_exact_tokens_for_tokens"), args);
    if amounts.is_empty() {
        panic!("empty router response");
    }
}

fn exec_aqua_edge(
    env: &Env,
    contract: &Address,
    aqua_router: &Address,
    pool_tokens: &Vec<Address>,
    pool_index: &BytesN<32>,
    token_in: &Address,
    token_out: &Address,
    amount_in: i128,
) {
    if amount_in <= 0 {
        panic!("hop amount must be positive");
    }
    let transfer_args: Vec<Val> = vec![
        env,
        contract.into_val(env),
        aqua_router.into_val(env),
        amount_in.into_val(env),
    ];
    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: token_in.clone(),
                fn_name: symbol_short!("transfer"),
                args: transfer_args,
            },
            sub_invocations: vec![env],
        }),
    ]);
    let hop: Vec<Val> = vec![
        env,
        pool_tokens.into_val(env),
        pool_index.into_val(env),
        token_out.into_val(env),
    ];
    let swaps_chain: Vec<Val> = vec![env, hop.into_val(env)];
    let args: Vec<Val> = vec![
        env,
        contract.into_val(env),
        swaps_chain.into_val(env),
        token_in.into_val(env),
        (amount_in as u128).into_val(env),
        0u128.into_val(env),
    ];
    let _reported: u128 =
        env.invoke_contract(aqua_router, &Symbol::new(env, "swap_chained"), args);
}

fn exec_phoenix_edge(
    env: &Env,
    contract: &Address,
    pool: &Address,
    token_in: &Address,
    amount_in: i128,
    deadline: u64,
) {
    if amount_in <= 0 {
        panic!("hop amount must be positive");
    }
    let transfer_args: Vec<Val> = vec![
        env,
        contract.into_val(env),
        pool.into_val(env),
        amount_in.into_val(env),
    ];
    env.authorize_as_current_contract(vec![
        env,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: token_in.clone(),
                fn_name: symbol_short!("transfer"),
                args: transfer_args,
            },
            sub_invocations: vec![env],
        }),
    ]);
    // Phoenix swap has 7 params. The per-hop minimum and spread stay None
    // because the plan-level guard enforces the minimum on the summed
    // output, but the deadline is passed through: the contract already
    // checked it, and the venue should not outlive that check either.
    let none_val: Val = ().into_val(env);
    let deadline_val: Val = if deadline == 0 {
        none_val
    } else {
        Some(deadline).into_val(env)
    };
    let args: Vec<Val> = vec![
        env,
        contract.into_val(env),
        token_in.into_val(env),
        amount_in.into_val(env),
        none_val,     // ask_asset_min_amount
        none_val,     // max_spread_bps
        deadline_val, // deadline
        none_val,     // max_allowed_fee_bps
    ];
    let _reported: i128 = env.invoke_contract(pool, &Symbol::new(env, "swap"), args);
}
