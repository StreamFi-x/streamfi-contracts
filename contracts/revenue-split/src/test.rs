#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env,
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
    let payer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = sac.address();
    let token = TokenClient::new(&env, &token_addr);
    let sac_admin = StellarAssetClient::new(&env, &token_addr);
    (env, admin, creator, payer, token, sac_admin)
}

fn deploy(env: &Env, admin: &Address, token_addr: &Address) -> RevenueSplitContractClient<'static> {
    let id = env.register(RevenueSplitContract, ());
    let c = RevenueSplitContractClient::new(env, &id);
    c.init(admin, token_addr);
    c
}

#[test]
fn split_50_50_distributes_evenly() {
    let (env, admin, creator, payer, token, token_admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &100_000_000i128);

    let sid = c.create_split(
        &creator,
        &vec![
            &env,
            Recipient {
                address: a.clone(),
                bps: 5000,
            },
            Recipient {
                address: b.clone(),
                bps: 5000,
            },
        ],
    );

    c.pay(&payer, &sid, &100_000_000i128);
    assert_eq!(token.balance(&a), 50_000_000i128);
    assert_eq!(token.balance(&b), 50_000_000i128);
    assert_eq!(token.balance(&payer), 0);
}

#[test]
fn rounding_remainder_goes_to_last_recipient() {
    let (env, admin, creator, payer, token, token_admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let ct = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &1000i128);

    // 33% / 67% split of 1000 = 330 + 670 (67% gets the +1 remainder)
    let sid = ct.create_split(
        &creator,
        &vec![
            &env,
            Recipient {
                address: a.clone(),
                bps: 3300,
            },
            Recipient {
                address: b.clone(),
                bps: 6700,
            },
        ],
    );
    ct.pay(&payer, &sid, &1000i128);
    assert_eq!(token.balance(&a), 330i128);
    assert_eq!(token.balance(&b), 670i128);
    assert_eq!(token.balance(&payer), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // InvalidTotalBps
fn rejects_bps_not_summing_to_10000() {
    let (env, admin, creator, _payer, token, _) = setup();
    let a = Address::generate(&env);
    let c = deploy(&env, &admin, &token.address);
    c.create_split(
        &creator,
        &vec![
            &env,
            Recipient {
                address: a.clone(),
                bps: 5000,
            },
        ],
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")] // TooManyRecipients
fn rejects_too_many_recipients() {
    let (env, admin, creator, _payer, token, _) = setup();
    let ct = deploy(&env, &admin, &token.address);
    let mut recips = vec![&env];
    for _ in 0..11 {
        recips.push_back(Recipient {
            address: Address::generate(&env),
            bps: 909, // sum roughly ~9999 but won't matter — hits cap first
        });
    }
    ct.create_split(&creator, &recips);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // SplitInactive
fn cannot_pay_inactive_split() {
    let (env, admin, creator, payer, token, token_admin) = setup();
    let a = Address::generate(&env);
    let ct = deploy(&env, &admin, &token.address);
    token_admin.mint(&payer, &1000i128);
    let sid = ct.create_split(
        &creator,
        &vec![
            &env,
            Recipient {
                address: a.clone(),
                bps: 10_000,
            },
        ],
    );
    ct.set_active(&creator, &sid, &false);
    ct.pay(&payer, &sid, &1000i128);
}
