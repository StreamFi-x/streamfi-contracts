#![no_std]

//! StreamFi Subscription Contract
//!
//! A per-creator subscription system with on-chain tier management, one-time
//! time-bounded purchases, and access checks that can be called from the
//! StreamFi backend before issuing signed Mux playback tokens.
//!
//! Stellar does not support recurring billing natively — this contract models
//! subscriptions as one-time, time-bounded purchases. Renewal is a fresh
//! subscribe() call.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, String, Vec,
};

/// Errors returned by this contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SubError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    TierNotFound = 3,
    TierInactive = 4,
    NotCreator = 5,
    AlreadyActive = 6,
    NotSubscribed = 7,
    Unauthorized = 8,
    InvalidPrice = 9,
    InvalidDuration = 10,
}

/// A subscription tier defined by a creator.
#[contracttype]
#[derive(Clone)]
pub struct Tier {
    pub id: u32,
    pub creator: Address,
    pub name: String,
    /// Price in the smallest unit of the payment token (e.g. stroops for USDC).
    pub price: i128,
    /// How long a purchase of this tier lasts, in seconds.
    pub duration_secs: u64,
    /// Freeform perks description (kept on-chain for provenance).
    pub perks: String,
    pub active: bool,
}

/// An active subscription (one per (subscriber, creator) pair).
#[contracttype]
#[derive(Clone)]
pub struct Subscription {
    pub subscriber: Address,
    pub creator: Address,
    pub tier_id: u32,
    pub started_at: u64,
    pub expires_at: u64,
    pub cancelled: bool,
}

/// Persistent storage keys.
#[contracttype]
pub enum DataKey {
    /// Contract admin (can pause the whole contract).
    Admin,
    /// The token contract used for payments (e.g. USDC on Stellar).
    PaymentToken,
    /// Monotonically increasing tier id counter.
    NextTierId,
    /// A tier by id.
    Tier(u32),
    /// The list of tier ids owned by a creator.
    CreatorTiers(Address),
    /// A subscription indexed by (subscriber, creator).
    Sub(Address, Address),
}

// ── Events ────────────────────────────────────────────────────────────────

#[contractevent]
pub struct TierCreated {
    #[topic]
    pub creator: Address,
    pub tier_id: u32,
    pub name: String,
    pub price: i128,
    pub duration_secs: u64,
}

#[contractevent]
pub struct TierUpdated {
    #[topic]
    pub creator: Address,
    pub tier_id: u32,
    pub active: bool,
}

#[contractevent]
pub struct Subscribed {
    #[topic]
    pub subscriber: Address,
    #[topic]
    pub creator: Address,
    pub tier_id: u32,
    pub started_at: u64,
    pub expires_at: u64,
}

#[contractevent]
pub struct SubCancelled {
    #[topic]
    pub subscriber: Address,
    #[topic]
    pub creator: Address,
    pub expires_at: u64,
}

// ── Contract ──────────────────────────────────────────────────────────────

#[contract]
pub struct SubscriptionContract;

#[contractimpl]
impl SubscriptionContract {
    /// One-time initialization. Sets admin and the payment token address.
    pub fn init(env: Env, admin: Address, payment_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, SubError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PaymentToken, &payment_token);
        env.storage().instance().set(&DataKey::NextTierId, &1u32);
    }

    /// Creator defines a new subscription tier.
    /// Requires the creator's auth.
    pub fn create_tier(
        env: Env,
        creator: Address,
        name: String,
        price: i128,
        duration_secs: u64,
        perks: String,
    ) -> u32 {
        creator.require_auth();
        Self::require_initialized(&env);

        if price <= 0 {
            panic_with_error!(&env, SubError::InvalidPrice);
        }
        if duration_secs == 0 {
            panic_with_error!(&env, SubError::InvalidDuration);
        }

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTierId)
            .unwrap_or(1u32);
        env.storage()
            .instance()
            .set(&DataKey::NextTierId, &(id + 1));

        let tier = Tier {
            id,
            creator: creator.clone(),
            name: name.clone(),
            price,
            duration_secs,
            perks,
            active: true,
        };
        env.storage().persistent().set(&DataKey::Tier(id), &tier);

        // Append tier id to creator's tier list.
        let mut list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorTiers(creator.clone()))
            .unwrap_or(Vec::new(&env));
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorTiers(creator.clone()), &list);

        TierCreated {
            creator,
            tier_id: id,
            name,
            price,
            duration_secs,
        }
        .publish(&env);

        id
    }

    /// Creator toggles a tier's active status. Existing subs are unaffected.
    pub fn set_tier_active(env: Env, creator: Address, tier_id: u32, active: bool) {
        creator.require_auth();

        let mut tier: Tier = env
            .storage()
            .persistent()
            .get(&DataKey::Tier(tier_id))
            .unwrap_or_else(|| panic_with_error!(&env, SubError::TierNotFound));

        if tier.creator != creator {
            panic_with_error!(&env, SubError::NotCreator);
        }

        tier.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Tier(tier_id), &tier);

        TierUpdated {
            creator,
            tier_id,
            active,
        }
        .publish(&env);
    }

    /// Viewer subscribes to a creator at the given tier.
    /// Transfers `tier.price` from subscriber to creator using the payment
    /// token, then records the subscription with an expiry timestamp.
    pub fn subscribe(env: Env, subscriber: Address, creator: Address, tier_id: u32) {
        subscriber.require_auth();
        Self::require_initialized(&env);

        let tier: Tier = env
            .storage()
            .persistent()
            .get(&DataKey::Tier(tier_id))
            .unwrap_or_else(|| panic_with_error!(&env, SubError::TierNotFound));

        if !tier.active {
            panic_with_error!(&env, SubError::TierInactive);
        }
        if tier.creator != creator {
            panic_with_error!(&env, SubError::TierNotFound);
        }

        // Refuse if the subscriber already has an active (non-expired) sub
        // to this creator. Explicit cancel-and-resub is required to change tier.
        let now = env.ledger().timestamp();
        if let Some(existing) = env
            .storage()
            .persistent()
            .get::<DataKey, Subscription>(&DataKey::Sub(subscriber.clone(), creator.clone()))
        {
            if !existing.cancelled && existing.expires_at > now {
                panic_with_error!(&env, SubError::AlreadyActive);
            }
        }

        // Pay: transfer tier.price of payment_token from subscriber to creator.
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, SubError::NotInitialized));
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&subscriber, &creator, &tier.price);

        let sub = Subscription {
            subscriber: subscriber.clone(),
            creator: creator.clone(),
            tier_id,
            started_at: now,
            expires_at: now + tier.duration_secs,
            cancelled: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Sub(subscriber.clone(), creator.clone()), &sub);

        Subscribed {
            subscriber,
            creator,
            tier_id,
            started_at: sub.started_at,
            expires_at: sub.expires_at,
        }
        .publish(&env);
    }

    /// Subscriber flags a subscription as cancelled. Access remains until expires_at
    /// (they already paid for that time).
    pub fn cancel(env: Env, subscriber: Address, creator: Address) {
        subscriber.require_auth();

        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&DataKey::Sub(subscriber.clone(), creator.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, SubError::NotSubscribed));

        if sub.subscriber != subscriber {
            panic_with_error!(&env, SubError::Unauthorized);
        }

        sub.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Sub(subscriber.clone(), creator.clone()), &sub);

        SubCancelled {
            subscriber,
            creator,
            expires_at: sub.expires_at,
        }
        .publish(&env);
    }

    /// Read-only: is this subscription currently active?
    /// Used by StreamFi backend before issuing a signed Mux playback token.
    pub fn is_active(env: Env, subscriber: Address, creator: Address) -> bool {
        match env
            .storage()
            .persistent()
            .get::<DataKey, Subscription>(&DataKey::Sub(subscriber, creator))
        {
            Some(s) => s.expires_at > env.ledger().timestamp(),
            None => false,
        }
    }

    /// Read-only: fetch a subscription record (if any).
    pub fn get_subscription(
        env: Env,
        subscriber: Address,
        creator: Address,
    ) -> Option<Subscription> {
        env.storage()
            .persistent()
            .get(&DataKey::Sub(subscriber, creator))
    }

    /// Read-only: fetch a tier.
    pub fn get_tier(env: Env, tier_id: u32) -> Option<Tier> {
        env.storage().persistent().get(&DataKey::Tier(tier_id))
    }

    /// Read-only: list a creator's tiers.
    pub fn creator_tiers(env: Env, creator: Address) -> Vec<Tier> {
        let ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorTiers(creator))
            .unwrap_or(Vec::new(&env));

        let mut out = Vec::new(&env);
        for id in ids.iter() {
            if let Some(t) = env
                .storage()
                .persistent()
                .get::<DataKey, Tier>(&DataKey::Tier(id))
            {
                out.push_back(t);
            }
        }
        out
    }

    // ── internal helpers ────────────────────────────────────────────────

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, SubError::NotInitialized);
        }
    }
}

#[cfg(test)]
mod test;
