#![no_std]

//! StreamFi Revenue Split Contract
//!
//! Atomically split incoming payments (tips, subscriptions) between multiple
//! recipients for multi-host streams. Creator defines a split of basis points
//! (out of 10,000) across recipients; a payer calls pay() which routes the
//! payment through the contract into each recipient's balance in one call.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, Vec,
};

/// Basis points denominator (100.00%).
const TOTAL_BPS: u32 = 10_000;
/// Cap on the number of recipients in one split.
const MAX_RECIPIENTS: u32 = 10;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SplitError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    SplitNotFound = 3,
    NotCreator = 4,
    InvalidRecipients = 5,
    InvalidTotalBps = 6,
    TooManyRecipients = 7,
    SplitInactive = 8,
    InvalidAmount = 9,
    ZeroAddress = 10,
}

#[contracttype]
#[derive(Clone)]
pub struct Recipient {
    pub address: Address,
    pub bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct Split {
    pub id: u32,
    pub creator: Address,
    pub recipients: Vec<Recipient>,
    pub active: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    PaymentToken,
    NextSplitId,
    Split(u32),
    CreatorSplits(Address),
}

// ── Events ────────────────────────────────────────────────────────────────

#[contractevent]
pub struct SplitCreated {
    #[topic]
    pub creator: Address,
    pub split_id: u32,
    pub recipient_count: u32,
}

#[contractevent]
pub struct SplitUpdated {
    #[topic]
    pub creator: Address,
    pub split_id: u32,
    pub active: bool,
}

#[contractevent]
pub struct Paid {
    #[topic]
    pub payer: Address,
    #[topic]
    pub split_id: u32,
    pub total: i128,
}

// ── Contract ──────────────────────────────────────────────────────────────

#[contract]
pub struct RevenueSplitContract;

#[contractimpl]
impl RevenueSplitContract {
    pub fn init(env: Env, admin: Address, payment_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, SplitError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PaymentToken, &payment_token);
        env.storage().instance().set(&DataKey::NextSplitId, &1u32);
    }

    /// Creator defines a new split. The recipient basis-point shares must sum
    /// to exactly TOTAL_BPS (10,000). Cap at MAX_RECIPIENTS.
    pub fn create_split(env: Env, creator: Address, recipients: Vec<Recipient>) -> u32 {
        creator.require_auth();
        Self::require_initialized(&env);

        Self::validate_recipients(&env, &recipients);

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextSplitId)
            .unwrap_or(1u32);
        env.storage()
            .instance()
            .set(&DataKey::NextSplitId, &(id + 1));

        let split = Split {
            id,
            creator: creator.clone(),
            recipients: recipients.clone(),
            active: true,
        };
        env.storage().persistent().set(&DataKey::Split(id), &split);

        let mut list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorSplits(creator.clone()))
            .unwrap_or(Vec::new(&env));
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorSplits(creator.clone()), &list);

        SplitCreated {
            creator,
            split_id: id,
            recipient_count: recipients.len(),
        }
        .publish(&env);
        id
    }

    /// Creator activates or deactivates a split.
    pub fn set_active(env: Env, creator: Address, split_id: u32, active: bool) {
        creator.require_auth();
        let mut split: Split = env
            .storage()
            .persistent()
            .get(&DataKey::Split(split_id))
            .unwrap_or_else(|| panic_with_error!(&env, SplitError::SplitNotFound));
        if split.creator != creator {
            panic_with_error!(&env, SplitError::NotCreator);
        }
        split.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Split(split_id), &split);
        SplitUpdated {
            creator,
            split_id,
            active,
        }
        .publish(&env);
    }

    /// Payer sends `amount` into the split, routed atomically to all recipients.
    pub fn pay(env: Env, payer: Address, split_id: u32, amount: i128) {
        payer.require_auth();
        Self::require_initialized(&env);

        if amount <= 0 {
            panic_with_error!(&env, SplitError::InvalidAmount);
        }

        let split: Split = env
            .storage()
            .persistent()
            .get(&DataKey::Split(split_id))
            .unwrap_or_else(|| panic_with_error!(&env, SplitError::SplitNotFound));
        if !split.active {
            panic_with_error!(&env, SplitError::SplitInactive);
        }

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, SplitError::NotInitialized));
        let token = token::Client::new(&env, &token_addr);

        // Compute per-recipient share. Give any bps rounding remainder to the
        // last recipient so the total transferred equals `amount` exactly.
        let mut distributed: i128 = 0;
        let last_index = split.recipients.len() - 1;
        for i in 0..split.recipients.len() {
            let r = split.recipients.get(i).unwrap();
            let share = if i == last_index {
                amount - distributed
            } else {
                (amount * (r.bps as i128)) / (TOTAL_BPS as i128)
            };
            distributed += share;
            if share > 0 {
                token.transfer(&payer, &r.address, &share);
            }
        }

        Paid {
            payer,
            split_id,
            total: amount,
        }
        .publish(&env);
    }

    // ── Read-only accessors ─────────────────────────────────────────────

    pub fn get_split(env: Env, split_id: u32) -> Option<Split> {
        env.storage().persistent().get(&DataKey::Split(split_id))
    }

    pub fn creator_splits(env: Env, creator: Address) -> Vec<Split> {
        let ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorSplits(creator))
            .unwrap_or(Vec::new(&env));
        let mut out = Vec::new(&env);
        for id in ids.iter() {
            if let Some(s) = env
                .storage()
                .persistent()
                .get::<DataKey, Split>(&DataKey::Split(id))
            {
                out.push_back(s);
            }
        }
        out
    }

    // ── internal helpers ────────────────────────────────────────────────

    fn validate_recipients(env: &Env, recipients: &Vec<Recipient>) {
        if recipients.is_empty() {
            panic_with_error!(env, SplitError::InvalidRecipients);
        }
        if recipients.len() > MAX_RECIPIENTS {
            panic_with_error!(env, SplitError::TooManyRecipients);
        }
        let mut sum: u32 = 0;
        for r in recipients.iter() {
            if r.bps == 0 {
                panic_with_error!(env, SplitError::InvalidRecipients);
            }
            sum += r.bps;
        }
        if sum != TOTAL_BPS {
            panic_with_error!(env, SplitError::InvalidTotalBps);
        }
    }

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, SplitError::NotInitialized);
        }
    }
}

#[cfg(test)]
mod test;
