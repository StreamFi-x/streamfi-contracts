#![no_std]

//! StreamFi Token-Gated Access Contract
//!
//! Per-creator access "gates" that viewers purchase. Purchase transfers the
//! gate price in the payment token from viewer to creator, and grants the
//! viewer an on-chain access record. The StreamFi backend calls the read-only
//! has_access(gate_id, holder) before issuing a signed Mux JWT for gated
//! streams.
//!
//! Access is optionally time-bounded (duration_secs > 0) or permanent
//! (duration_secs == 0). Creators can revoke access if needed.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, String, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GateError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    GateNotFound = 3,
    NotCreator = 4,
    InvalidPrice = 5,
    GateInactive = 6,
    AlreadyHasAccess = 7,
    NoAccess = 8,
}

#[contracttype]
#[derive(Clone)]
pub struct Gate {
    pub id: u32,
    pub creator: Address,
    pub name: String,
    pub price: i128,
    /// 0 = permanent access; > 0 = seconds of access per purchase.
    pub duration_secs: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct AccessRecord {
    pub gate_id: u32,
    pub holder: Address,
    pub granted_at: u64,
    /// 0 = permanent; otherwise unix seconds when access ends.
    pub expires_at: u64,
    pub revoked: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    PaymentToken,
    NextGateId,
    Gate(u32),
    CreatorGates(Address),
    Access(u32, Address),
}

// ── Events ────────────────────────────────────────────────────────────────

#[contractevent]
pub struct GateCreated {
    #[topic]
    pub creator: Address,
    pub gate_id: u32,
    pub price: i128,
    pub duration_secs: u64,
}

#[contractevent]
pub struct GateUpdated {
    #[topic]
    pub creator: Address,
    pub gate_id: u32,
    pub active: bool,
}

#[contractevent]
pub struct AccessPurchased {
    #[topic]
    pub holder: Address,
    #[topic]
    pub gate_id: u32,
    pub granted_at: u64,
    pub expires_at: u64,
}

#[contractevent]
pub struct AccessRevoked {
    #[topic]
    pub holder: Address,
    #[topic]
    pub gate_id: u32,
}

// ── Contract ──────────────────────────────────────────────────────────────

#[contract]
pub struct TokenGatedAccessContract;

#[contractimpl]
impl TokenGatedAccessContract {
    pub fn init(env: Env, admin: Address, payment_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, GateError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PaymentToken, &payment_token);
        env.storage().instance().set(&DataKey::NextGateId, &1u32);
    }

    pub fn create_gate(
        env: Env,
        creator: Address,
        name: String,
        price: i128,
        duration_secs: u64,
    ) -> u32 {
        creator.require_auth();
        Self::require_initialized(&env);

        if price <= 0 {
            panic_with_error!(&env, GateError::InvalidPrice);
        }

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextGateId)
            .unwrap_or(1u32);
        env.storage()
            .instance()
            .set(&DataKey::NextGateId, &(id + 1));

        let gate = Gate {
            id,
            creator: creator.clone(),
            name,
            price,
            duration_secs,
            active: true,
        };
        env.storage().persistent().set(&DataKey::Gate(id), &gate);

        let mut list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorGates(creator.clone()))
            .unwrap_or(Vec::new(&env));
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorGates(creator.clone()), &list);

        GateCreated {
            creator,
            gate_id: id,
            price,
            duration_secs,
        }
        .publish(&env);
        id
    }

    pub fn set_gate_active(env: Env, creator: Address, gate_id: u32, active: bool) {
        creator.require_auth();
        let mut gate: Gate = env
            .storage()
            .persistent()
            .get(&DataKey::Gate(gate_id))
            .unwrap_or_else(|| panic_with_error!(&env, GateError::GateNotFound));
        if gate.creator != creator {
            panic_with_error!(&env, GateError::NotCreator);
        }
        gate.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Gate(gate_id), &gate);
        GateUpdated {
            creator,
            gate_id,
            active,
        }
        .publish(&env);
    }

    /// Viewer purchases access. Refuses if they already have active,
    /// non-revoked, non-expired access (explicit renewal is a separate call).
    pub fn purchase(env: Env, holder: Address, gate_id: u32) {
        holder.require_auth();
        Self::require_initialized(&env);

        let gate: Gate = env
            .storage()
            .persistent()
            .get(&DataKey::Gate(gate_id))
            .unwrap_or_else(|| panic_with_error!(&env, GateError::GateNotFound));
        if !gate.active {
            panic_with_error!(&env, GateError::GateInactive);
        }

        let now = env.ledger().timestamp();
        if let Some(existing) = env
            .storage()
            .persistent()
            .get::<DataKey, AccessRecord>(&DataKey::Access(gate_id, holder.clone()))
        {
            let still_valid =
                !existing.revoked && (existing.expires_at == 0 || existing.expires_at > now);
            if still_valid {
                panic_with_error!(&env, GateError::AlreadyHasAccess);
            }
        }

        // Pay creator.
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap_or_else(|| panic_with_error!(&env, GateError::NotInitialized));
        let token = token::Client::new(&env, &token_addr);
        token.transfer(&holder, &gate.creator, &gate.price);

        let expires_at = if gate.duration_secs == 0 {
            0
        } else {
            now + gate.duration_secs
        };
        let rec = AccessRecord {
            gate_id,
            holder: holder.clone(),
            granted_at: now,
            expires_at,
            revoked: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Access(gate_id, holder.clone()), &rec);

        AccessPurchased {
            holder,
            gate_id,
            granted_at: now,
            expires_at,
        }
        .publish(&env);
    }

    /// Creator revokes a holder's access.
    pub fn revoke(env: Env, creator: Address, gate_id: u32, holder: Address) {
        creator.require_auth();
        let gate: Gate = env
            .storage()
            .persistent()
            .get(&DataKey::Gate(gate_id))
            .unwrap_or_else(|| panic_with_error!(&env, GateError::GateNotFound));
        if gate.creator != creator {
            panic_with_error!(&env, GateError::NotCreator);
        }
        let mut rec: AccessRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Access(gate_id, holder.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, GateError::NoAccess));
        rec.revoked = true;
        env.storage()
            .persistent()
            .set(&DataKey::Access(gate_id, holder.clone()), &rec);
        AccessRevoked { holder, gate_id }.publish(&env);
    }

    /// Read-only access check used by the StreamFi backend.
    pub fn has_access(env: Env, gate_id: u32, holder: Address) -> bool {
        match env
            .storage()
            .persistent()
            .get::<DataKey, AccessRecord>(&DataKey::Access(gate_id, holder))
        {
            Some(r) => !r.revoked && (r.expires_at == 0 || r.expires_at > env.ledger().timestamp()),
            None => false,
        }
    }

    pub fn get_gate(env: Env, gate_id: u32) -> Option<Gate> {
        env.storage().persistent().get(&DataKey::Gate(gate_id))
    }

    pub fn get_access(env: Env, gate_id: u32, holder: Address) -> Option<AccessRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Access(gate_id, holder))
    }

    pub fn creator_gates(env: Env, creator: Address) -> Vec<Gate> {
        let ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorGates(creator))
            .unwrap_or(Vec::new(&env));
        let mut out = Vec::new(&env);
        for id in ids.iter() {
            if let Some(g) = env
                .storage()
                .persistent()
                .get::<DataKey, Gate>(&DataKey::Gate(id))
            {
                out.push_back(g);
            }
        }
        out
    }

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, GateError::NotInitialized);
        }
    }
}

#[cfg(test)]
mod test;
