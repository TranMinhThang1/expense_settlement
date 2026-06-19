#![no_std]
//! Expense Settlement
//!
//! An on-chain expense-report ledger for small businesses. Employees submit
//! claims (amount + category + IPFS/hash of the receipt); a designated
//! manager approves or rejects each claim; an approved claim is queued for
//! a finance officer to mark as settled once the off-chain disbursement is
//! actually paid. All state transitions are timestamped and authorisation
//! is enforced via `require_auth()` plus an on-chain role check, so the
//! audit trail is tamper-evident.
//!
//! This contract intentionally does NOT move XLM or any SAC token — it is
//! a record-keeping / workflow contract. Real payouts are settled by the
//! finance team out-of-band and recorded back here via `mark_settled`.

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, BytesN, Env, String, Symbol,
};

// -------------------------------------------------------------------------
// Status codes
// -------------------------------------------------------------------------
/// Claim has been submitted by an employee and is awaiting manager review.
pub const STATUS_PENDING: u32 = 0;
/// Claim was approved by the manager — queued for finance to settle.
pub const STATUS_APPROVED: u32 = 1;
/// Claim was rejected by the manager — terminal.
pub const STATUS_REJECTED: u32 = 2;
/// Claim has been paid out by finance — terminal.
pub const STATUS_SETTLED: u32 = 3;

// -------------------------------------------------------------------------
// Storage keys
// -------------------------------------------------------------------------
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Address allowed to (re)assign roles.
    Admin,
    /// Address allowed to approve / reject claims.
    Manager,
    /// Address allowed to mark approved claims as settled.
    Finance,
    /// Monotonically increasing claim id counter (u32).
    Counter,
    /// Persistent storage of each claim, keyed by its id.
    Claim(u32),
}

// -------------------------------------------------------------------------
// Domain types
// -------------------------------------------------------------------------
/// A single expense claim record stored on-chain.
#[derive(Clone)]
#[contracttype]
pub struct Claim {
    /// Employee who submitted the claim.
    pub employee: Address,
    /// Claim amount in the smallest unit of the company's reporting currency
    /// (e.g. cents). Stored as `i128` so it interops with Stellar's native
    /// numeric type even though negative values are rejected.
    pub amount: i128,
    /// Free-form category tag — e.g. `TRAVEL`, `MEALS`, `SUPPLIES`.
    pub category: Symbol,
    /// 32-byte content hash of the receipt artifact (e.g. SHA-256 of the
    /// PDF or the IPFS CID-v0 digest). The artifact itself lives off-chain.
    pub receipt_hash: BytesN<32>,
    /// Current lifecycle status — see `STATUS_*` constants.
    pub status: u32,
    /// Reviewer note. Empty until the manager rejects (then it carries the
    /// rejection reason) or the finance officer settles.
    pub note: String,
    /// Ledger timestamp (seconds since epoch) at which the claim was filed.
    pub submitted_at: u64,
    /// Ledger timestamp of the most recent state transition.
    pub updated_at: u64,
}

// -------------------------------------------------------------------------
// Contract
// -------------------------------------------------------------------------
#[contract]
pub struct ExpenseSettlement;

#[contractimpl]
impl ExpenseSettlement {
    /// One-shot initializer. Registers the three privileged roles
    /// (`admin`, `manager`, `finance`) and resets the claim counter.
    /// Panics if the contract was already initialized.
    pub fn init(env: Env, admin: Address, manager: Address, finance: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Manager, &manager);
        env.storage().instance().set(&DataKey::Finance, &finance);
        env.storage().instance().set(&DataKey::Counter, &0u32);
    }

    /// Admin rotates the manager or finance role.
    /// `role` must be the symbol `MANAGER` or `FINANCE`.
    pub fn set_role(env: Env, admin: Address, role: Symbol, new_addr: Address) {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        if stored_admin != admin {
            panic!("only admin may rotate roles");
        }

        let manager_sym = Symbol::new(&env, "MANAGER");
        let finance_sym = Symbol::new(&env, "FINANCE");
        if role == manager_sym {
            env.storage().instance().set(&DataKey::Manager, &new_addr);
        } else if role == finance_sym {
            env.storage().instance().set(&DataKey::Finance, &new_addr);
        } else {
            panic!("unknown role");
        }
    }

    /// Employee files a new expense claim. Returns the freshly assigned
    /// `claim_id`. The amount must be strictly positive; the receipt hash
    /// is recorded verbatim and is the anchor for the off-chain proof.
    pub fn submit_claim(
        env: Env,
        employee: Address,
        amount: i128,
        category: Symbol,
        receipt_hash: BytesN<32>,
    ) -> u32 {
        employee.require_auth();
        if amount <= 0 {
            panic!("amount must be positive");
        }

        let prev: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Counter)
            .unwrap_or(0u32);
        let claim_id = prev
            .checked_add(1)
            .expect("claim id overflow");

        let now = env.ledger().timestamp();
        let claim = Claim {
            employee: employee.clone(),
            amount,
            category,
            receipt_hash,
            status: STATUS_PENDING,
            note: String::from_str(&env, ""),
            submitted_at: now,
            updated_at: now,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);
        env.storage().instance().set(&DataKey::Counter, &claim_id);

        claim_id
    }

    /// Manager approves a pending claim — moves it into the settlement
    /// queue. Only the registered manager address can succeed.
    pub fn approve_claim(env: Env, manager: Address, claim_id: u32) {
        manager.require_auth();
        Self::assert_role(&env, &manager, &DataKey::Manager, "only manager may approve");

        let mut claim: Claim = env
            .storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .expect("claim not found");
        if claim.status != STATUS_PENDING {
            panic!("claim is not pending");
        }
        claim.status = STATUS_APPROVED;
        claim.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);
    }

    /// Manager rejects a pending claim and stores the human-readable
    /// `reason` so the employee can see why it was denied.
    pub fn reject_claim(env: Env, manager: Address, claim_id: u32, reason: String) {
        manager.require_auth();
        Self::assert_role(&env, &manager, &DataKey::Manager, "only manager may reject");

        let mut claim: Claim = env
            .storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .expect("claim not found");
        if claim.status != STATUS_PENDING {
            panic!("claim is not pending");
        }
        claim.status = STATUS_REJECTED;
        claim.note = reason;
        claim.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);
    }

    /// Finance officer records that an approved claim has actually been
    /// paid out (the disbursement itself happens off-chain via the
    /// company's payroll rails). Only an `APPROVED` claim can be settled.
    pub fn mark_settled(env: Env, finance: Address, claim_id: u32) {
        finance.require_auth();
        Self::assert_role(&env, &finance, &DataKey::Finance, "only finance may settle");

        let mut claim: Claim = env
            .storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .expect("claim not found");
        if claim.status != STATUS_APPROVED {
            panic!("claim must be approved before settlement");
        }
        claim.status = STATUS_SETTLED;
        claim.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);
    }

    /// Total number of claims ever submitted (also the highest claim_id).
    pub fn claim_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::Counter)
            .unwrap_or(0u32)
    }

    /// Current lifecycle status of a claim — one of the `STATUS_*` codes.
    pub fn claim_status(env: Env, claim_id: u32) -> u32 {
        let claim: Claim = env
            .storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .expect("claim not found");
        claim.status
    }

    /// Fetch a full claim record by id.
    pub fn get_claim(env: Env, claim_id: u32) -> Claim {
        env.storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .expect("claim not found")
    }

    /// Return the currently registered manager address.
    pub fn get_manager(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Manager)
            .expect("not initialized")
    }

    /// Return the currently registered finance officer address.
    pub fn get_finance(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Finance)
            .expect("not initialized")
    }

    // ---------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------
    fn assert_role(env: &Env, caller: &Address, key: &DataKey, msg: &str) {
        let expected: Address = env
            .storage()
            .instance()
            .get(key)
            .expect("not initialized");
        if &expected != caller {
            panic!("{}", msg);
        }
    }
}
