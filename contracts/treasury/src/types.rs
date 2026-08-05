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

