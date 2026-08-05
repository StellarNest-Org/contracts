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

        let list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::MemberList(treasury_id))
            .unwrap_or_else(|| Vec::new(&env));
        let mut updated = Vec::new(&env);
        for addr in list.iter() {
            if addr != member {
                updated.push_back(addr);
            }
        }
        env.storage()
            .persistent()
            .set(&DataKey::MemberList(treasury_id), &updated);
        Ok(())
    }

    pub fn get_member(env: Env, treasury_id: u64, member: Address) -> Result<Member, Error> {
        load_member(&env, treasury_id, &member)
    }

    pub fn list_members(env: Env, treasury_id: u64) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::MemberList(treasury_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ---------------------------------------------------------------
    // Rules engine
    // ---------------------------------------------------------------

    pub fn set_approval_rule(
        env: Env,
        treasury_id: u64,
        caller: Address,
        approval_threshold: i128,
        required_approvals: u32,
    ) -> Result<(), Error> {
        caller.require_auth();
        let mut treasury = load_treasury(&env, treasury_id)?;
        require_role(&env, &treasury, &caller, Role::can_administer)?;
        treasury.approval_threshold = approval_threshold;
        treasury.required_approvals = required_approvals.max(1);
        env.storage()
            .persistent()
            .set(&DataKey::Treasury(treasury_id), &treasury);
        Ok(())
    }

    // ---------------------------------------------------------------
    // Deposits & withdrawals
    // ---------------------------------------------------------------

    pub fn deposit(env: Env, treasury_id: u64, from: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let mut treasury = load_treasury(&env, treasury_id)?;

        let token_client = token::Client::new(&env, &treasury.asset);
        token_client.transfer(&from, &env.current_contract_address(), &amount);

        treasury.balance += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Treasury(treasury_id), &treasury);
        Ok(())
    }

    /// Request a withdrawal. If the amount is within the caller's spending limit and below the
    /// treasury's approval threshold, it executes immediately. Otherwise it opens a pending
    /// withdrawal that must collect `required_approvals` from Owner/Parent/Guardian members.
    pub fn request_withdrawal(
        env: Env,
        treasury_id: u64,
        caller: Address,
        to: Address,
        amount: i128,
    ) -> Result<u64, Error> {
        caller.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let treasury = load_treasury(&env, treasury_id)?;
        if treasury.frozen {
            return Err(Error::TreasuryFrozen);
        }
        if treasury.balance < amount {
            return Err(Error::InsufficientBalance);
        }
        let member = load_member(&env, treasury_id, &caller)?;
        if !member.role.can_spend() {
            return Err(Error::NotAuthorized);
        }
        if let Some(limit) = member.spending_limit {
            if amount > limit {
                return Err(Error::SpendingLimitExceeded);
            }
        }

        let id = next_id(&env, &DataKey::NextWithdrawalId);

        if amount < treasury.approval_threshold {
            execute_transfer(&env, treasury_id, &to, amount)?;
            let record = Withdrawal {
                id,
                treasury_id,
                requested_by: caller,
                to,
                amount,
                approvals: Vec::new(&env),
                executed: true,
            };
            env.storage()
                .persistent()
                .set(&DataKey::Withdrawal(id), &record);
        } else {
            let record = Withdrawal {
                id,
                treasury_id,
                requested_by: caller,
                to,
                amount,
                approvals: Vec::new(&env),
                executed: false,
            };
            env.storage()
                .persistent()
                .set(&DataKey::Withdrawal(id), &record);
        }
        Ok(id)
    }

    pub fn approve_withdrawal(
        env: Env,
        withdrawal_id: u64,
        approver: Address,
    ) -> Result<(), Error> {
        approver.require_auth();
        let mut withdrawal: Withdrawal = env
            .storage()
            .persistent()
            .get(&DataKey::Withdrawal(withdrawal_id))
            .ok_or(Error::WithdrawalNotFound)?;

        if withdrawal.executed {
            return Err(Error::WithdrawalAlreadyExecuted);
        }
        let treasury = load_treasury(&env, withdrawal.treasury_id)?;
        if treasury.frozen {
            return Err(Error::TreasuryFrozen);
        }
        let member = load_member(&env, withdrawal.treasury_id, &approver)?;
        if !member.role.can_approve() {
            return Err(Error::NotAuthorized);
        }
        if withdrawal.approvals.contains(&approver) {
            return Err(Error::AlreadyApproved);
        }
        withdrawal.approvals.push_back(approver);

        if withdrawal.approvals.len() >= treasury.required_approvals {
            execute_transfer(
                &env,
                withdrawal.treasury_id,
                &withdrawal.to,
                withdrawal.amount,
            )?;
            withdrawal.executed = true;
        }
        env.storage()
            .persistent()
            .set(&DataKey::Withdrawal(withdrawal_id), &withdrawal);
        Ok(())
    }

    pub fn get_withdrawal(env: Env, withdrawal_id: u64) -> Result<Withdrawal, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Withdrawal(withdrawal_id))
            .ok_or(Error::WithdrawalNotFound)
    }

    // ---------------------------------------------------------------
    // Savings goals
    // ---------------------------------------------------------------

    pub fn create_savings_goal(
        env: Env,
        treasury_id: u64,
        caller: Address,
        name: String,
        target_amount: i128,
    ) -> Result<u64, Error> {
        caller.require_auth();
        let treasury = load_treasury(&env, treasury_id)?;
        require_role(&env, &treasury, &caller, Role::can_administer)?;

        let id = next_id(&env, &DataKey::NextGoalId);
        let goal = SavingsGoal {
            id,
            treasury_id,
            name,
            target_amount,
            current_amount: 0,
            created_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&DataKey::Goal(id), &goal);

        let mut list: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::GoalList(treasury_id))
            .unwrap_or_else(|| Vec::new(&env));
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::GoalList(treasury_id), &list);
        Ok(id)
    }

    pub fn contribute_to_goal(
        env: Env,
        goal_id: u64,
        from: Address,
        amount: i128,
    ) -> Result<(), Error> {
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let mut goal: SavingsGoal = env
            .storage()
            .persistent()
            .get(&DataKey::Goal(goal_id))
            .ok_or(Error::GoalNotFound)?;
        let mut treasury = load_treasury(&env, goal.treasury_id)?;

        let token_client = token::Client::new(&env, &treasury.asset);
        token_client.transfer(&from, &env.current_contract_address(), &amount);

        treasury.balance += amount;
        goal.current_amount += amount;

        env.storage()
            .persistent()
            .set(&DataKey::Treasury(goal.treasury_id), &treasury);
        env.storage()
            .persistent()
            .set(&DataKey::Goal(goal_id), &goal);
        Ok(())
    }

    pub fn get_savings_goal(env: Env, goal_id: u64) -> Result<SavingsGoal, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Goal(goal_id))
            .ok_or(Error::GoalNotFound)
    }

    pub fn list_savings_goals(env: Env, treasury_id: u64) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::GoalList(treasury_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ---------------------------------------------------------------
    // Bills & recurring payments (also used for child allowances)
    // ---------------------------------------------------------------

    pub fn create_bill(
        env: Env,
        treasury_id: u64,
        caller: Address,
        name: String,
        payee: Address,
        amount: i128,
        interval_ledgers: u32,
    ) -> Result<u64, Error> {
        caller.require_auth();
        let treasury = load_treasury(&env, treasury_id)?;
        require_role(&env, &treasury, &caller, Role::can_administer)?;

        let id = next_id(&env, &DataKey::NextBillId);
        let bill = Bill {
            id,
            treasury_id,
            name,
            payee,
            amount,
            interval_ledgers,
            next_due_ledger: env.ledger().sequence() + interval_ledgers,
            active: true,
        };
        env.storage().persistent().set(&DataKey::Bill(id), &bill);

        let mut list: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::BillList(treasury_id))
            .unwrap_or_else(|| Vec::new(&env));
        list.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::BillList(treasury_id), &list);
        Ok(id)
    }

    /// Executes a due bill. Callable by anyone (typically a relayer/cron), but only pays out if
    /// the schedule says it's due, keeping payment timing on-chain and tamper-proof.
    pub fn pay_bill(env: Env, bill_id: u64) -> Result<(), Error> {
        let mut bill: Bill = env
            .storage()
            .persistent()
            .get(&DataKey::Bill(bill_id))
            .ok_or(Error::BillNotFound)?;
        if !bill.active {
            return Err(Error::BillNotFound);
        }
        if env.ledger().sequence() < bill.next_due_ledger {
            return Err(Error::BillNotDue);
        }
        execute_transfer(&env, bill.treasury_id, &bill.payee, bill.amount)?;
        bill.next_due_ledger = env.ledger().sequence() + bill.interval_ledgers;
        env.storage()
            .persistent()
            .set(&DataKey::Bill(bill_id), &bill);
        Ok(())
    }

    pub fn cancel_bill(env: Env, bill_id: u64, caller: Address) -> Result<(), Error> {
        caller.require_auth();
        let mut bill: Bill = env
            .storage()
            .persistent()
            .get(&DataKey::Bill(bill_id))
            .ok_or(Error::BillNotFound)?;
        let treasury = load_treasury(&env, bill.treasury_id)?;
        require_role(&env, &treasury, &caller, Role::can_administer)?;
        bill.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Bill(bill_id), &bill);
        Ok(())
    }

    pub fn get_bill(env: Env, bill_id: u64) -> Result<Bill, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Bill(bill_id))
            .ok_or(Error::BillNotFound)
    }

    pub fn list_bills(env: Env, treasury_id: u64) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::BillList(treasury_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ---------------------------------------------------------------
    // Inheritance vault
    // ---------------------------------------------------------------

