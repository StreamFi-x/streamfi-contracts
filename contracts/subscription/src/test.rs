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
    let subscriber = Address::generate(&env);

    // Deploy a Stellar asset contract to act as USDC.
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = sac.address();
    let token = TokenClient::new(&env, &token_addr);
    let token_admin_client = StellarAssetClient::new(&env, &token_addr);

    (
        env,
        admin,
        creator,
        subscriber,
        token_addr,
        token,
        token_admin_client,
    )
}

fn deploy_sub(env: &Env, admin: &Address, token: &Address) -> SubscriptionContractClient<'static> {
    let contract_id = env.register(SubscriptionContract, ());
    let client = SubscriptionContractClient::new(env, &contract_id);
    client.init(admin, token);
    client
}

#[test]
fn create_tier_stores_and_lists() {
    let (env, admin, creator, _sub, token_addr, _t, _ta) = setup();
    let sub = deploy_sub(&env, &admin, &token_addr);

    let id = sub.create_tier(
        &creator,
        &String::from_str(&env, "Supporter"),
        &10_000_000i128, // 1 USDC (7 decimals)
        &(30 * 86400u64),
        &String::from_str(&env, "Custom emotes"),
    );
    assert_eq!(id, 1);

    let tiers = sub.creator_tiers(&creator);
    assert_eq!(tiers.len(), 1);
    let t = tiers.get(0).unwrap();
    assert_eq!(t.id, 1);
    assert_eq!(t.creator, creator);
    assert!(t.active);
}

#[test]
fn subscribe_transfers_and_grants_access() {
    let (env, admin, creator, viewer, token_addr, token, token_admin) = setup();
    let sub = deploy_sub(&env, &admin, &token_addr);

    // Give viewer some USDC.
    token_admin.mint(&viewer, &50_000_000i128); // 5 USDC

    let id = sub.create_tier(
        &creator,
        &String::from_str(&env, "Supporter"),
        &10_000_000i128,
        &(30 * 86400u64),
        &String::from_str(&env, ""),
    );

    // No sub yet.
    assert!(!sub.is_active(&viewer, &creator));

    sub.subscribe(&viewer, &creator, &id);

    // Access granted, funds moved.
    assert!(sub.is_active(&viewer, &creator));
    assert_eq!(token.balance(&viewer), 40_000_000i128);
    assert_eq!(token.balance(&creator), 10_000_000i128);

    let rec = sub.get_subscription(&viewer, &creator).unwrap();
    assert_eq!(rec.tier_id, id);
    assert!(!rec.cancelled);
    assert_eq!(rec.expires_at, rec.started_at + 30 * 86400);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // AlreadyActive
fn subscribe_rejects_duplicate_active() {
    let (env, admin, creator, viewer, token_addr, _, token_admin) = setup();
    let sub = deploy_sub(&env, &admin, &token_addr);
    token_admin.mint(&viewer, &50_000_000i128);

    let id = sub.create_tier(
        &creator,
        &String::from_str(&env, "S"),
        &10_000_000i128,
        &(30 * 86400u64),
        &String::from_str(&env, ""),
    );
    sub.subscribe(&viewer, &creator, &id);
    sub.subscribe(&viewer, &creator, &id); // should panic
}

#[test]
fn expiry_flips_access_off() {
    let (env, admin, creator, viewer, token_addr, _, token_admin) = setup();
    let sub = deploy_sub(&env, &admin, &token_addr);
    token_admin.mint(&viewer, &50_000_000i128);

    let id = sub.create_tier(
        &creator,
        &String::from_str(&env, "S"),
        &10_000_000i128,
        &(30 * 86400u64),
        &String::from_str(&env, ""),
    );
    sub.subscribe(&viewer, &creator, &id);
    assert!(sub.is_active(&viewer, &creator));

    // Warp ledger past expiry.
    let new_ts = env.ledger().timestamp() + 31 * 86400;
    env.ledger().set_timestamp(new_ts);

    assert!(!sub.is_active(&viewer, &creator));
}

#[test]
fn cancel_keeps_access_until_expiry() {
    let (env, admin, creator, viewer, token_addr, _, token_admin) = setup();
    let sub = deploy_sub(&env, &admin, &token_addr);
    token_admin.mint(&viewer, &50_000_000i128);

    let id = sub.create_tier(
        &creator,
        &String::from_str(&env, "S"),
        &10_000_000i128,
        &(30 * 86400u64),
        &String::from_str(&env, ""),
    );
    sub.subscribe(&viewer, &creator, &id);
    sub.cancel(&viewer, &creator);

    // Still active because cancel does not expire immediately.
    assert!(sub.is_active(&viewer, &creator));
    let rec = sub.get_subscription(&viewer, &creator).unwrap();
    assert!(rec.cancelled);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // TierInactive
fn subscribe_rejects_inactive_tier() {
    let (env, admin, creator, viewer, token_addr, _, token_admin) = setup();
    let sub = deploy_sub(&env, &admin, &token_addr);
    token_admin.mint(&viewer, &50_000_000i128);

    let id = sub.create_tier(
        &creator,
        &String::from_str(&env, "S"),
        &10_000_000i128,
        &(30 * 86400u64),
        &String::from_str(&env, ""),
    );
    sub.set_tier_active(&creator, &id, &false);
    sub.subscribe(&viewer, &creator, &id);
}
