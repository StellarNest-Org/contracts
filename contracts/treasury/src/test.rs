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

