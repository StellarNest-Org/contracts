#![no_std]

mod errors;
mod types;

#[cfg(test)]
mod test;

use errors::Error;
use soroban_sdk::{contract, contractimpl, token, Address, Env, String, Vec};
use types::{Bill, DataKey, InheritanceVault, Member, Role, SavingsGoal, Treasury, Withdrawal};

#[contract]
pub struct TreasuryContract;

#[contractimpl]
impl TreasuryContract {
    // ---------------------------------------------------------------
    // Treasury lifecycle
    // ---------------------------------------------------------------

    /// Create a new family treasury denominated in `asset` (a Stellar Asset Contract address).
    /// `approval_threshold` / `required_approvals` gate withdrawals above that amount.
    pub fn create_treasury(
        env: Env,
        owner: Address,
        name: String,
        asset: Address,
        approval_threshold: i128,
        required_approvals: u32,
    ) -> u64 {
        owner.require_auth();

        let id = next_id(&env, &DataKey::NextTreasuryId);
        let treasury = Treasury {
            id,
            name,
            owner: owner.clone(),
            asset,
            balance: 0,
            frozen: false,
            approval_threshold,
            required_approvals: required_approvals.max(1),
            created_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Treasury(id), &treasury);

        let member = Member {
            role: Role::Owner,
            spending_limit: None,
            joined_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Member(id, owner.clone()), &member);
        let members: Vec<Address> = Vec::from_array(&env, [owner]);
        env.storage()
            .persistent()
            .set(&DataKey::MemberList(id), &members);

        id
    }

