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
    TokenClient<'static>,
    StellarAssetClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = sac.address();
    let token = TokenClient::new(&env, &token_addr);
    let sac_admin = StellarAssetClient::new(&env, &token_addr);
    (env, admin, creator, holder, token, sac_admin)
}

fn deploy(
    env: &Env,
    admin: &Address,
    token_addr: &Address,
) -> TokenGatedAccessContractClient<'static> {
    let id = env.register(TokenGatedAccessContract, ());
    let c = TokenGatedAccessContractClient::new(env, &id);
    c.init(admin, token_addr);
    c
}

#[test]
fn purchase_permanent_access_stays_valid() {
    let (env, admin, creator, holder, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&holder, &50_000_000i128);

    let gid = c.create_gate(
        &creator,
        &String::from_str(&env, "OG Pass"),
        &10_000_000i128,
        &0u64, // permanent
    );
    c.purchase(&holder, &gid);

    assert!(c.has_access(&gid, &holder));
    assert_eq!(token.balance(&creator), 10_000_000i128);
    // Long time later, still valid because permanent.
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 365 * 86400);
    assert!(c.has_access(&gid, &holder));
}

#[test]
fn timed_access_expires() {
    let (env, admin, creator, holder, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&holder, &50_000_000i128);
    let gid = c.create_gate(
        &creator,
        &String::from_str(&env, "Day Pass"),
        &10_000_000i128,
        &86400u64,
    );
    c.purchase(&holder, &gid);
    assert!(c.has_access(&gid, &holder));

    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 86400 + 1);
    assert!(!c.has_access(&gid, &holder));
}

#[test]
fn revoked_access_denied() {
    let (env, admin, creator, holder, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&holder, &50_000_000i128);
    let gid = c.create_gate(
        &creator,
        &String::from_str(&env, "Pass"),
        &10_000_000i128,
        &0u64,
    );
    c.purchase(&holder, &gid);
    assert!(c.has_access(&gid, &holder));
    c.revoke(&creator, &gid, &holder);
    assert!(!c.has_access(&gid, &holder));
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")] // AlreadyHasAccess
fn cannot_double_purchase_active_access() {
    let (env, admin, creator, holder, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&holder, &100_000_000i128);
    let gid = c.create_gate(
        &creator,
        &String::from_str(&env, "Pass"),
        &10_000_000i128,
        &0u64,
    );
    c.purchase(&holder, &gid);
    c.purchase(&holder, &gid); // panic
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // GateInactive
fn cannot_purchase_inactive_gate() {
    let (env, admin, creator, holder, token, token_admin) = setup();
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&holder, &50_000_000i128);
    let gid = c.create_gate(
        &creator,
        &String::from_str(&env, "P"),
        &10_000_000i128,
        &0u64,
    );
    c.set_gate_active(&creator, &gid, &false);
    c.purchase(&holder, &gid);
}
