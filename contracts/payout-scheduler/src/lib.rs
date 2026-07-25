#![no_std]

//! StreamFi Payout Scheduler Contract
//!
//! Creators configure a payout schedule (destination, minimum balance,
//! cadence). External payments flow into the schedule's pool via deposit().
//! Anyone can call trigger_payout() after the cadence has elapsed; if the
//! pool is at or above the minimum, the full pool balance sweeps to the
//! configured destination.
//!
//! This lets creators automate "sweep tips to my main wallet every 7 days
//! once I've accumulated at least X USDC" without operator intervention.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PayoutError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    ScheduleNotFound = 3,
    NotCreator = 4,
    InvalidMinBalance = 5,
    InvalidCadence = 6,
    InvalidAmount = 7,
    ScheduleInactive = 8,
    CadenceNotReached = 9,
    BelowMinimum = 10,
    NothingToPay = 11,
}

#[contracttype]
#[derive(Clone)]
pub struct Schedule {
    pub id: u32,
    pub creator: Address,
    pub destination: Address,
    pub min_balance: i128,
    pub cadence_secs: u64,
    pub last_payout_at: u64,
    pub pool_balance: i128,
    pub active: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    PaymentToken,
    NextScheduleId,
    Schedule(u32),
    CreatorSchedules(Address),
}

// ── Events ────────────────────────────────────────────────────────────────

#[contractevent]
pub struct ScheduleCreated {
    #[topic]
    pub creator: Address,
    pub schedule_id: u32,
    pub destination: Address,
    pub min_balance: i128,
    pub cadence_secs: u64,
}

#[contractevent]
pub struct ScheduleUpdated {
    #[topic]
    pub creator: Address,
    pub schedule_id: u32,
    pub active: bool,
}

#[contractevent]
pub struct Deposited {
    #[topic]
    pub payer: Address,
    #[topic]
    pub schedule_id: u32,
    pub amount: i128,
    pub new_pool_balance: i128,
}

#[contractevent]
pub struct PayoutTriggered {
    #[topic]
    pub schedule_id: u32,
    pub destination: Address,
    pub amount: i128,
    pub at: u64,
}

// ── Contract ──────────────────────────────────────────────────────────────

#[contract]
pub struct PayoutSchedulerContract;

#[contractimpl]
impl PayoutSchedulerContract {
    pub fn init(env: Env, admin: Address, payment_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, PayoutError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PaymentToken, &payment_token);
        env.storage()
            .instance()
            .set(&DataKey::NextScheduleId, &1u32);
    }

    /// Creator sets up a payout schedule.
    pub fn create_schedule(
        env: Env,
        creator: Address,
        destination: Address,
        min_balance: i128,
        cadence_secs: u64,
    ) -> u32 {
        creator.require_auth();
        Self::require_initialized(&env);

        if min_balance <= 0 {
            panic_with_error!(&env, PayoutError::InvalidMinBalance);
        }
        if cadence_secs == 0 {
            panic_with_error!(&env, PayoutError::InvalidCadence);
        }

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextScheduleId)
            .unwrap_or(1u32);
        env.storage()
            .instance()
            .set(&DataKey::NextScheduleId, &(id + 1));

        let sched = Schedule {
            id,
            creator: creator.clone(),
            destination,
            min_balance,
            cadence_secs,
            last_payout_at: env.ledger().timestamp(),
            pool_balance: 0,
            active: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Schedule(id), &sched);

        let mut list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorSchedules(creator.clone()))
            .unwrap_or(Vec::new(&env));
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorSchedules(creator.clone()), &list);

        ScheduleCreated {
            creator,
            schedule_id: id,
            destination: sched.destination,
            min_balance,
            cadence_secs,
        }
        .publish(&env);
        id
    }

    pub fn set_active(env: Env, creator: Address, schedule_id: u32, active: bool) {
        creator.require_auth();
        let mut sched: Schedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, PayoutError::ScheduleNotFound));
        if sched.creator != creator {
            panic_with_error!(&env, PayoutError::NotCreator);
        }
        sched.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Schedule(schedule_id), &sched);
        ScheduleUpdated {
            creator,
            schedule_id,
            active,
        }
        .publish(&env);
    }

    /// Anyone can deposit funds into a schedule's pool.
    pub fn deposit(env: Env, payer: Address, schedule_id: u32, amount: i128) {
        payer.require_auth();
        Self::require_initialized(&env);

        if amount <= 0 {
            panic_with_error!(&env, PayoutError::InvalidAmount);
        }

        let mut sched: Schedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, PayoutError::ScheduleNotFound));

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, PayoutError::NotInitialized));
        let token = token::Client::new(&env, &token_addr);
        let escrow = env.current_contract_address();
        token.transfer(&payer, &escrow, &amount);

        sched.pool_balance += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Schedule(schedule_id), &sched);

        Deposited {
            payer,
            schedule_id,
            amount,
            new_pool_balance: sched.pool_balance,
        }
        .publish(&env);
    }

    /// Permissionless: anyone can trigger a payout if the cadence has passed
    /// and the pool is at or above min_balance. Sweeps the full pool to the
    /// destination and resets last_payout_at.
    pub fn trigger_payout(env: Env, schedule_id: u32) {
        Self::require_initialized(&env);

        let mut sched: Schedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, PayoutError::ScheduleNotFound));

        if !sched.active {
            panic_with_error!(&env, PayoutError::ScheduleInactive);
        }

        let now = env.ledger().timestamp();
        if now < sched.last_payout_at + sched.cadence_secs {
            panic_with_error!(&env, PayoutError::CadenceNotReached);
        }
        if sched.pool_balance < sched.min_balance {
            panic_with_error!(&env, PayoutError::BelowMinimum);
        }
        if sched.pool_balance == 0 {
            panic_with_error!(&env, PayoutError::NothingToPay);
        }

        let payout_amount = sched.pool_balance;
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, PayoutError::NotInitialized));
        let token = token::Client::new(&env, &token_addr);
        let escrow = env.current_contract_address();
        token.transfer(&escrow, &sched.destination, &payout_amount);

        sched.pool_balance = 0;
        sched.last_payout_at = now;
        env.storage()
            .persistent()
            .set(&DataKey::Schedule(schedule_id), &sched);

        PayoutTriggered {
            schedule_id,
            destination: sched.destination,
            amount: payout_amount,
            at: now,
        }
        .publish(&env);
    }

    // ── Read-only accessors ─────────────────────────────────────────────

    pub fn get_schedule(env: Env, schedule_id: u32) -> Option<Schedule> {
        env.storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
    }

    pub fn creator_schedules(env: Env, creator: Address) -> Vec<Schedule> {
        let ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorSchedules(creator))
            .unwrap_or(Vec::new(&env));
        let mut out = Vec::new(&env);
        for id in ids.iter() {
            if let Some(s) = env
                .storage()
                .persistent()
                .get::<DataKey, Schedule>(&DataKey::Schedule(id))
            {
                out.push_back(s);
            }
        }
        out
    }

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, PayoutError::NotInitialized);
        }
    }
}

#[cfg(test)]
mod test;
