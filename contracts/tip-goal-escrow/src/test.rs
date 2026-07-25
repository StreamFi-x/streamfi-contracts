#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env, String,
};

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TokenClient<'static>,
    StellarAssetClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let tipper = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = sac.address();
    let token = TokenClient::new(&env, &token_addr);
    let token_admin_client = StellarAssetClient::new(&env, &token_addr);
    (
        env,
        admin,
        creator,
        tipper,
        token_addr,
        token,
        token_admin_client,
    )
}

fn deploy(env: &Env, admin: &Address, token: &Address) -> TipGoalContractClient<'static> {
    let id = env.register(TipGoalContract, ());
    let c = TipGoalContractClient::new(env, &id);
    c.init(admin, token);
    c
}

#[test]
fn happy_path_success_and_claim() {
    let (env, admin, creator, tipper, token_addr, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token_addr);
    token_admin.mint(&tipper, &100_000_000i128);

    let deadline = env.ledger().timestamp() + 7 * 86400;
    let gid = c.create_goal(
        &creator,
        &String::from_str(&env, "New mic"),
        &50_000_000i128,
        &deadline,
    );

    c.contribute(&tipper, &gid, &50_000_000i128);
    assert_eq!(c.contribution_of(&gid, &tipper), 50_000_000i128);

    env.ledger().set_timestamp(deadline + 1);
    c.finalize(&gid);

    let g = c.get_goal(&gid).unwrap();
    assert_eq!(g.status, GoalStatus::Success);

    c.claim(&creator, &gid);
    assert_eq!(token.balance(&creator), 50_000_000i128);
    assert_eq!(c.get_goal(&gid).unwrap().status, GoalStatus::Claimed);
}

#[test]
fn failure_path_refund() {
    let (env, admin, creator, tipper, token_addr, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token_addr);
    token_admin.mint(&tipper, &100_000_000i128);

    let deadline = env.ledger().timestamp() + 7 * 86400;
    let gid = c.create_goal(
        &creator,
        &String::from_str(&env, "T"),
        &100_000_000i128,
        &deadline,
    );

    c.contribute(&tipper, &gid, &30_000_000i128); // short of target

    env.ledger().set_timestamp(deadline + 1);
    c.finalize(&gid);
    assert_eq!(c.get_goal(&gid).unwrap().status, GoalStatus::Failed);

    c.refund(&tipper, &gid);
    assert_eq!(token.balance(&tipper), 100_000_000i128); // full amount back
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")] // AlreadyRefunded
fn refund_only_once() {
    let (env, admin, creator, tipper, token_addr, _, token_admin) = setup();
    let c = deploy(&env, &admin, &token_addr);
    token_admin.mint(&tipper, &100_000_000i128);

    let deadline = env.ledger().timestamp() + 3600;
    let gid = c.create_goal(
        &creator,
        &String::from_str(&env, "T"),
        &100_000_000i128,
        &deadline,
    );
    c.contribute(&tipper, &gid, &10_000_000i128);
    env.ledger().set_timestamp(deadline + 1);
    c.finalize(&gid);
    c.refund(&tipper, &gid);
    c.refund(&tipper, &gid); // panic
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")] // DeadlineNotReached
fn cannot_finalize_early() {
    let (env, admin, creator, _t, token_addr, _, _) = setup();
    let c = deploy(&env, &admin, &token_addr);
    let deadline = env.ledger().timestamp() + 3600;
    let gid = c.create_goal(&creator, &String::from_str(&env, "T"), &1i128, &deadline);
    c.finalize(&gid);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // NotSuccessful
fn cannot_claim_failed_goal() {
    let (env, admin, creator, _t, token_addr, _, _) = setup();
    let c = deploy(&env, &admin, &token_addr);
    let deadline = env.ledger().timestamp() + 3600;
    let gid = c.create_goal(
        &creator,
        &String::from_str(&env, "T"),
        &100_000_000i128,
        &deadline,
    );
    env.ledger().set_timestamp(deadline + 1);
    c.finalize(&gid);
    c.claim(&creator, &gid);
}
