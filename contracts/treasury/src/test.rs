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

