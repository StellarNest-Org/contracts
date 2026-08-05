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

    pub fn get_treasury(env: Env, treasury_id: u64) -> Result<Treasury, Error> {
        load_treasury(&env, treasury_id)
    }

    pub fn freeze_treasury(env: Env, treasury_id: u64, caller: Address) -> Result<(), Error> {
        caller.require_auth();
        let mut treasury = load_treasury(&env, treasury_id)?;
        require_role(&env, &treasury, &caller, |r| {
            r.can_administer() || matches!(r, Role::Guardian)
        })?;
        treasury.frozen = true;
        env.storage()
            .persistent()
            .set(&DataKey::Treasury(treasury_id), &treasury);
        Ok(())
    }

    pub fn unfreeze_treasury(env: Env, treasury_id: u64, caller: Address) -> Result<(), Error> {
        caller.require_auth();
        let mut treasury = load_treasury(&env, treasury_id)?;
        require_role(&env, &treasury, &caller, Role::can_administer)?;
        treasury.frozen = false;
        env.storage()
            .persistent()
            .set(&DataKey::Treasury(treasury_id), &treasury);
        Ok(())
    }

    // ---------------------------------------------------------------
    // Members & roles
    // ---------------------------------------------------------------

    pub fn add_member(
        env: Env,
        treasury_id: u64,
        caller: Address,
        member: Address,
        role: Role,
        spending_limit: Option<i128>,
    ) -> Result<(), Error> {
        caller.require_auth();
        let treasury = load_treasury(&env, treasury_id)?;
        require_role(&env, &treasury, &caller, Role::can_administer)?;

        if env
            .storage()
            .persistent()
            .has(&DataKey::Member(treasury_id, member.clone()))
        {
            return Err(Error::MemberAlreadyExists);
        }

        let record = Member {
            role,
            spending_limit,
            joined_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Member(treasury_id, member.clone()), &record);

        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::MemberList(treasury_id))
            .unwrap_or_else(|| Vec::new(&env));
        list.push_back(member);
        env.storage()
            .persistent()
            .set(&DataKey::MemberList(treasury_id), &list);
        Ok(())
    }

    pub fn remove_member(
        env: Env,
        treasury_id: u64,
        caller: Address,
        member: Address,
    ) -> Result<(), Error> {
        caller.require_auth();
        let treasury = load_treasury(&env, treasury_id)?;
        require_role(&env, &treasury, &caller, Role::can_administer)?;

        env.storage()
            .persistent()
            .remove(&DataKey::Member(treasury_id, member.clone()));

