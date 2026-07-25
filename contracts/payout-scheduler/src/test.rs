#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
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
    let destination = Address::generate(&env);
    let payer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = sac.address();
    let token = TokenClient::new(&env, &token_addr);
    let sac_admin = StellarAssetClient::new(&env, &token_addr);
    (env, admin, creator, destination, payer, token, sac_admin)
}

fn deploy(
    env: &Env,
    admin: &Address,
    token_addr: &Address,
) -> PayoutSchedulerContractClient<'static> {
    let id = env.register(PayoutSchedulerContract, ());
    let c = PayoutSchedulerContractClient::new(env, &id);
    c.init(admin, token_addr);
    c
}

#[test]
fn deposit_and_scheduled_payout() {
    let (env, admin, creator, destination, payer, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &100_000_000i128);

    let sid = c.create_schedule(&creator, &destination, &50_000_000i128, &(7 * 86400u64));

    c.deposit(&payer, &sid, &60_000_000i128);
    let sched = c.get_schedule(&sid).unwrap();
    assert_eq!(sched.pool_balance, 60_000_000i128);

    // Warp past cadence.
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 7 * 86400 + 1);
    c.trigger_payout(&sid);

    assert_eq!(token.balance(&destination), 60_000_000i128);
    let after = c.get_schedule(&sid).unwrap();
    assert_eq!(after.pool_balance, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")] // CadenceNotReached
fn cannot_trigger_before_cadence() {
    let (env, admin, creator, destination, payer, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &100_000_000i128);
    let sid = c.create_schedule(&creator, &destination, &10_000_000i128, &(7 * 86400u64));
    c.deposit(&payer, &sid, &20_000_000i128);
    c.trigger_payout(&sid); // panic — no time has passed
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // BelowMinimum
fn cannot_trigger_below_minimum() {
    let (env, admin, creator, destination, payer, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &100_000_000i128);
    let sid = c.create_schedule(&creator, &destination, &50_000_000i128, &(7 * 86400u64));
    c.deposit(&payer, &sid, &10_000_000i128); // below minimum
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 7 * 86400 + 1);
    c.trigger_payout(&sid);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // ScheduleInactive
fn cannot_trigger_inactive_schedule() {
    let (env, admin, creator, destination, payer, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &100_000_000i128);
    let sid = c.create_schedule(&creator, &destination, &10_000_000i128, &(7 * 86400u64));
    c.deposit(&payer, &sid, &20_000_000i128);
    c.set_active(&creator, &sid, &false);
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 7 * 86400 + 1);
    c.trigger_payout(&sid);
}

#[test]
fn multiple_cadence_cycles() {
    let (env, admin, creator, destination, payer, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &200_000_000i128);

    let sid = c.create_schedule(&creator, &destination, &10_000_000i128, &(86400u64));

    // Cycle 1
    c.deposit(&payer, &sid, &50_000_000i128);
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 86400 + 1);
    c.trigger_payout(&sid);
    assert_eq!(token.balance(&destination), 50_000_000i128);

    // Cycle 2
    c.deposit(&payer, &sid, &30_000_000i128);
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 86400 + 1);
    c.trigger_payout(&sid);
    assert_eq!(token.balance(&destination), 80_000_000i128);
}
