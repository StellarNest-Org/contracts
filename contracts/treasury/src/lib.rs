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

