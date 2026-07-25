#![no_std]

//! StreamFi Tip Goal Escrow Contract
//!
//! Kickstarter-style tip goals. Creator sets a target amount and a deadline.
//! Tippers contribute into the contract's escrow. After the deadline:
//! - If total contributions >= target: creator can claim the funds.
//! - If not: each contributor can refund their contribution.
//!
//! This gives tippers real confidence that a large tip toward a specific
//! creator outcome ("new mic", "charity drive") is either used as intended
//! or returned.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, String, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GoalError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    GoalNotFound = 3,
    NotCreator = 4,
    DeadlineInPast = 5,
    InvalidTarget = 6,
    InvalidAmount = 7,
    DeadlinePassed = 8,
    DeadlineNotReached = 9,
    NotFinalized = 10,
    AlreadyFinalized = 11,
    NotSuccessful = 12,
    NotFailed = 13,
    AlreadyClaimed = 14,
    NoContribution = 15,
    AlreadyRefunded = 16,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GoalStatus {
    Active = 0,
    Success = 1,
    Failed = 2,
    Claimed = 3,
}

#[contracttype]
#[derive(Clone)]
pub struct Goal {
    pub id: u32,
    pub creator: Address,
    pub title: String,
    pub target: i128,
    pub raised: i128,
    pub deadline: u64,
    pub status: GoalStatus,
}

#[contracttype]
pub enum DataKey {
    Admin,
    PaymentToken,
    NextGoalId,
    Goal(u32),
    CreatorGoals(Address),
    Contribution(u32, Address),
    GoalContributors(u32),
    Refunded(u32, Address),
}

// ── Events ────────────────────────────────────────────────────────────────

#[contractevent]
pub struct GoalCreated {
    #[topic]
    pub creator: Address,
    pub goal_id: u32,
    pub target: i128,
    pub deadline: u64,
}

#[contractevent]
pub struct Contributed {
    #[topic]
    pub tipper: Address,
    #[topic]
    pub goal_id: u32,
    pub amount: i128,
    pub new_total: i128,
}

#[contractevent]
pub struct GoalFinalized {
    #[topic]
    pub goal_id: u32,
    pub success: bool,
    pub raised: i128,
    pub target: i128,
}

#[contractevent]
pub struct GoalClaimed {
    #[topic]
    pub creator: Address,
    #[topic]
    pub goal_id: u32,
    pub amount: i128,
}

#[contractevent]
pub struct Refunded {
    #[topic]
    pub tipper: Address,
    #[topic]
    pub goal_id: u32,
    pub amount: i128,
}

// ── Contract ──────────────────────────────────────────────────────────────

#[contract]
pub struct TipGoalContract;

#[contractimpl]
impl TipGoalContract {
    pub fn init(env: Env, admin: Address, payment_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, GoalError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PaymentToken, &payment_token);
        env.storage().instance().set(&DataKey::NextGoalId, &1u32);
    }

    /// Creator creates a new tip goal.
    pub fn create_goal(
        env: Env,
        creator: Address,
        title: String,
        target: i128,
        deadline: u64,
    ) -> u32 {
        creator.require_auth();
        Self::require_initialized(&env);

        if target <= 0 {
            panic_with_error!(&env, GoalError::InvalidTarget);
        }
        if deadline <= env.ledger().timestamp() {
            panic_with_error!(&env, GoalError::DeadlineInPast);
        }

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextGoalId)
            .unwrap_or(1u32);
        env.storage()
            .instance()
            .set(&DataKey::NextGoalId, &(id + 1));

        let goal = Goal {
            id,
            creator: creator.clone(),
            title,
            target,
            raised: 0,
            deadline,
            status: GoalStatus::Active,
        };
        env.storage().persistent().set(&DataKey::Goal(id), &goal);

        let mut list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorGoals(creator.clone()))
            .unwrap_or(Vec::new(&env));
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorGoals(creator.clone()), &list);

        GoalCreated {
            creator,
            goal_id: id,
            target,
            deadline,
        }
        .publish(&env);
        id
    }

    /// Tipper contributes to a goal. Funds are moved into the contract's escrow.
    pub fn contribute(env: Env, tipper: Address, goal_id: u32, amount: i128) {
        tipper.require_auth();
        Self::require_initialized(&env);

        if amount <= 0 {
            panic_with_error!(&env, GoalError::InvalidAmount);
        }

        let mut goal: Goal = env
            .storage()
            .persistent()
            .get(&DataKey::Goal(goal_id))
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::GoalNotFound));

        if goal.status != GoalStatus::Active {
            panic_with_error!(&env, GoalError::AlreadyFinalized);
        }
        if env.ledger().timestamp() >= goal.deadline {
            panic_with_error!(&env, GoalError::DeadlinePassed);
        }

        // Transfer amount from tipper into this contract.
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::NotInitialized));
        let token = token::Client::new(&env, &token_addr);
        let escrow = env.current_contract_address();
        token.transfer(&tipper, &escrow, &amount);

        // Update per-contributor tally.
        let key = DataKey::Contribution(goal_id, tipper.clone());
        let prior: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(prior + amount));

        // Track first-time contributors so refund iteration works.
        if prior == 0 {
            let mut contributors: Vec<Address> = env
                .storage()
                .persistent()
                .get(&DataKey::GoalContributors(goal_id))
                .unwrap_or(Vec::new(&env));
            contributors.push_back(tipper.clone());
            env.storage()
                .persistent()
                .set(&DataKey::GoalContributors(goal_id), &contributors);
        }

        goal.raised += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Goal(goal_id), &goal);

        Contributed {
            tipper,
            goal_id,
            amount,
            new_total: goal.raised,
        }
        .publish(&env);
    }

    /// After the deadline, anyone can call finalize() to lock the goal's status.
    /// This does not move funds; use claim() (success) or refund() (failed).
    pub fn finalize(env: Env, goal_id: u32) {
        let mut goal: Goal = env
            .storage()
            .persistent()
            .get(&DataKey::Goal(goal_id))
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::GoalNotFound));

        if goal.status != GoalStatus::Active {
            panic_with_error!(&env, GoalError::AlreadyFinalized);
        }
        if env.ledger().timestamp() < goal.deadline {
            panic_with_error!(&env, GoalError::DeadlineNotReached);
        }

        let success = goal.raised >= goal.target;
        goal.status = if success {
            GoalStatus::Success
        } else {
            GoalStatus::Failed
        };
        env.storage()
            .persistent()
            .set(&DataKey::Goal(goal_id), &goal);

        GoalFinalized {
            goal_id,
            success,
            raised: goal.raised,
            target: goal.target,
        }
        .publish(&env);
    }

    /// Creator withdraws raised funds on a successful goal.
    pub fn claim(env: Env, creator: Address, goal_id: u32) {
        creator.require_auth();

        let mut goal: Goal = env
            .storage()
            .persistent()
            .get(&DataKey::Goal(goal_id))
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::GoalNotFound));

        if goal.creator != creator {
            panic_with_error!(&env, GoalError::NotCreator);
        }
        if goal.status == GoalStatus::Claimed {
            panic_with_error!(&env, GoalError::AlreadyClaimed);
        }
        if goal.status != GoalStatus::Success {
            panic_with_error!(&env, GoalError::NotSuccessful);
        }

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::NotInitialized));
        let token = token::Client::new(&env, &token_addr);
        let amount = goal.raised;
        let escrow = env.current_contract_address();
        token.transfer(&escrow, &creator, &amount);

        goal.status = GoalStatus::Claimed;
        env.storage()
            .persistent()
            .set(&DataKey::Goal(goal_id), &goal);

        GoalClaimed {
            creator,
            goal_id,
            amount,
        }
        .publish(&env);
    }

    /// Contributor refunds their contribution on a failed goal.
    pub fn refund(env: Env, tipper: Address, goal_id: u32) {
        tipper.require_auth();

        let goal: Goal = env
            .storage()
            .persistent()
            .get(&DataKey::Goal(goal_id))
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::GoalNotFound));

        if goal.status != GoalStatus::Failed {
            panic_with_error!(&env, GoalError::NotFailed);
        }

        let refunded_key = DataKey::Refunded(goal_id, tipper.clone());
        if env.storage().persistent().has(&refunded_key) {
            panic_with_error!(&env, GoalError::AlreadyRefunded);
        }

        let contrib_key = DataKey::Contribution(goal_id, tipper.clone());
        let amount: i128 = env
            .storage()
            .persistent()
            .get(&contrib_key)
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::NoContribution));

        if amount <= 0 {
            panic_with_error!(&env, GoalError::NoContribution);
        }

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, GoalError::NotInitialized));
        let token = token::Client::new(&env, &token_addr);
        let escrow = env.current_contract_address();
        token.transfer(&escrow, &tipper, &amount);

        env.storage().persistent().set(&refunded_key, &true);

        Refunded {
            tipper,
            goal_id,
            amount,
        }
        .publish(&env);
    }

    // ── Read-only accessors ─────────────────────────────────────────────

    pub fn get_goal(env: Env, goal_id: u32) -> Option<Goal> {
        env.storage().persistent().get(&DataKey::Goal(goal_id))
    }

    pub fn creator_goals(env: Env, creator: Address) -> Vec<Goal> {
        let ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorGoals(creator))
            .unwrap_or(Vec::new(&env));
        let mut out = Vec::new(&env);
        for id in ids.iter() {
            if let Some(g) = env
                .storage()
                .persistent()
                .get::<DataKey, Goal>(&DataKey::Goal(id))
            {
                out.push_back(g);
            }
        }
        out
    }

    pub fn contribution_of(env: Env, goal_id: u32, tipper: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Contribution(goal_id, tipper))
            .unwrap_or(0)
    }

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, GoalError::NotInitialized);
        }
    }
}

#[cfg(test)]
mod test;
