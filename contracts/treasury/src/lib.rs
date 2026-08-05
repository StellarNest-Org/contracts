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

