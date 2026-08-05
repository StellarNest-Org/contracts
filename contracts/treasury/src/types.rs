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

