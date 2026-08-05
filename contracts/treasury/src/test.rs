#![cfg(test)]

use crate::types::Beneficiary;
use crate::{errors::Error, TreasuryContract, TreasuryContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env, String, Vec};

fn setup() -> (Env, TreasuryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);

    let asset_admin = Address::generate(&env);
    let sac_id = env.register_stellar_asset_contract_v2(asset_admin);
    let asset = sac_id.address();

    (env, client, asset)
}

fn mint(env: &Env, asset: &Address, to: &Address, amount: i128) {
    token::StellarAssetClient::new(env, asset).mint(to, &amount);
}

fn balance_of(env: &Env, asset: &Address, who: &Address) -> i128 {
    token::Client::new(env, asset).balance(who)
}

#[test]
fn create_treasury_registers_owner() {
    let (env, client, asset) = setup();
    let owner = Address::generate(&env);
    let id = client.create_treasury(
        &owner,
        &String::from_str(&env, "Adeyemi Family"),
        &asset,
        &1_000,
        &2,
    );
    let treasury = client.get_treasury(&id);
    assert_eq!(treasury.owner, owner);
    assert_eq!(treasury.balance, 0);
    let members = client.list_members(&id);
    assert_eq!(members.len(), 1);
}

#[test]
fn deposit_increases_balance() {
    let (env, client, asset) = setup();
    let owner = Address::generate(&env);
    let id = client.create_treasury(
        &owner,
        &String::from_str(&env, "Family"),
        &asset,
        &1_000,
        &2,
    );

    mint(&env, &asset, &owner, 5_000);
    client.deposit(&id, &owner, &2_000);

    let treasury = client.get_treasury(&id);
    assert_eq!(treasury.balance, 2_000);
}

#[test]
fn small_withdrawal_executes_immediately() {
    let (env, client, asset) = setup();
    let owner = Address::generate(&env);
    let child = Address::generate(&env);
    let id = client.create_treasury(
        &owner,
        &String::from_str(&env, "Family"),
        &asset,
        &1_000,
        &2,
    );

    mint(&env, &asset, &owner, 5_000);
    client.deposit(&id, &owner, &5_000);

    let withdrawal_id = client.request_withdrawal(&id, &owner, &child, &500);
    let withdrawal = client.get_withdrawal(&withdrawal_id);
    assert!(withdrawal.executed);
    assert_eq!(balance_of(&env, &asset, &child), 500);
    assert_eq!(client.get_treasury(&id).balance, 4_500);
}

#[test]
fn large_withdrawal_requires_approvals() {
    let (env, client, asset) = setup();
    let owner = Address::generate(&env);
    let parent2 = Address::generate(&env);
    let to = Address::generate(&env);
    let id = client.create_treasury(
        &owner,
        &String::from_str(&env, "Family"),
        &asset,
        &1_000,
        &2,
    );
    client.add_member(&id, &owner, &parent2, &crate::types::Role::Parent, &None);

    mint(&env, &asset, &owner, 10_000);
    client.deposit(&id, &owner, &10_000);

    let withdrawal_id = client.request_withdrawal(&id, &owner, &to, &5_000);
    let pending = client.get_withdrawal(&withdrawal_id);
    assert!(!pending.executed);
    assert_eq!(balance_of(&env, &asset, &to), 0);

    client.approve_withdrawal(&withdrawal_id, &owner);
    let still_pending = client.get_withdrawal(&withdrawal_id);
    assert!(!still_pending.executed);

    client.approve_withdrawal(&withdrawal_id, &parent2);
    let done = client.get_withdrawal(&withdrawal_id);
    assert!(done.executed);
    assert_eq!(balance_of(&env, &asset, &to), 5_000);
}

#[test]
fn double_approval_rejected() {
    let (env, client, asset) = setup();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let id = client.create_treasury(
        &owner,
        &String::from_str(&env, "Family"),
        &asset,
        &1_000,
        &2,
    );
    mint(&env, &asset, &owner, 10_000);
    client.deposit(&id, &owner, &10_000);
    let withdrawal_id = client.request_withdrawal(&id, &owner, &to, &5_000);
    client.approve_withdrawal(&withdrawal_id, &owner);
    let result = client.try_approve_withdrawal(&withdrawal_id, &owner);
    assert_eq!(result, Err(Ok(Error::AlreadyApproved)));
}

#[test]
fn spending_limit_enforced_for_child() {
    let (env, client, asset) = setup();
    let owner = Address::generate(&env);
    let child = Address::generate(&env);
    let to = Address::generate(&env);
    let id = client.create_treasury(
        &owner,
        &String::from_str(&env, "Family"),
        &asset,
        &1_000,
        &2,
    );
    client.add_member(&id, &owner, &child, &crate::types::Role::Child, &Some(100));
    mint(&env, &asset, &owner, 10_000);
    client.deposit(&id, &owner, &10_000);

    let result = client.try_request_withdrawal(&id, &child, &to, &200);
    assert_eq!(result, Err(Ok(Error::SpendingLimitExceeded)));

    let withdrawal_id = client.request_withdrawal(&id, &child, &to, &50);
    assert!(client.get_withdrawal(&withdrawal_id).executed);
}

#[test]
fn frozen_treasury_blocks_withdrawals() {
    let (env, client, asset) = setup();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let id = client.create_treasury(
        &owner,
        &String::from_str(&env, "Family"),
        &asset,
        &1_000,
        &2,
    );
    mint(&env, &asset, &owner, 10_000);
    client.deposit(&id, &owner, &10_000);

