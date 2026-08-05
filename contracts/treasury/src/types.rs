use soroban_sdk::{contracttype, Address, String, Vec};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Owner,
    Parent,
    Guardian,
    Child,
    Advisor,
    Viewer,
}

impl Role {
    /// Roles that can manage members, rules, bills and goals.
    pub fn can_administer(&self) -> bool {
        matches!(self, Role::Owner | Role::Parent)
    }

    /// Roles that can approve a pending withdrawal.
    pub fn can_approve(&self) -> bool {
        matches!(self, Role::Owner | Role::Parent | Role::Guardian)
    }

    /// Roles that can initiate a spend request at all.
    pub fn can_spend(&self) -> bool {
        !matches!(self, Role::Viewer)
    }
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Member {
    pub role: Role,
    /// Per-period spending limit in the treasury's default asset. None = unlimited (subject to rules).
    pub spending_limit: Option<i128>,
    pub joined_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Treasury {
    pub id: u64,
    pub name: String,
    pub owner: Address,
    pub asset: Address,
    pub balance: i128,
    pub frozen: bool,
    pub approval_threshold: i128,
    pub required_approvals: u32,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SavingsGoal {
    pub id: u64,
    pub treasury_id: u64,
    pub name: String,
    pub target_amount: i128,
    pub current_amount: i128,
    pub created_at: u64,
}

