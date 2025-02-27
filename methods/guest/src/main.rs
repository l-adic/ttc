// Copyright 2024 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unused_doc_comments)]
#![no_main]

use alloy_primitives::{Address, U256};
use alloy_sol_types::{SolValue, sol};
use risc0_steel::{
    ethereum::{EthEvmInput, ETH_SEPOLIA_CHAIN_SPEC},
    Commitment, Contract,
};
use risc0_zkvm::guest::env;
use hashbrown::HashMap;
use ttc::strict::{self, Preferences};

risc0_zkvm::guest::entry!(main);


sol!(
    #[sol(all_derives)]
    TopTradingCycle,
    "../../contract/out/TopTradingCycle.sol/TopTradingCycle.json"
);

use core::fmt;

impl fmt::Debug for TopTradingCycle::TokenReallocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenReallocation")
            .field("tokenId", &self.tokenId)
            .field("newOwner", &self.newOwner)
            .finish()
    }
}

sol! {
    #[derive(Debug)]
    struct Journal {
        Commitment commitment;
        address ttcContract;
        TopTradingCycle.TokenReallocation[] reallocations;
    }
}

fn build_owner_dict(prefs: &[TopTradingCycle::TokenPreferences]) -> HashMap<U256, Address> {
    prefs
        .iter()
        .cloned()
        .map(|tp| (tp.tokenId, tp.owner))
        .collect()
}

// This function calls the solver and produces the data we need to
// submit to the contract
fn reallocate(
    depositor_address_from_token_id: HashMap<U256, Address>,
    prefs: Vec<TopTradingCycle::TokenPreferences>,
) -> Vec<TopTradingCycle::TokenReallocation> {
    let prefs = {
        let ps = prefs
            .into_iter()
            .map(
                |TopTradingCycle::TokenPreferences {
                     tokenId,
                     preferences,
                     ..
                 }| { (tokenId, preferences) },
            )
            .collect();
        Preferences::new(ps).unwrap()
    };
    let mut g = strict::PreferenceGraph::new(prefs).unwrap();
    let alloc = strict::Allocation::from(g.solve_preferences().unwrap());
    alloc
        .allocation
        .into_iter()
        .map(|(new_owner, token_id)| {
            let new_owner = depositor_address_from_token_id
                .get(&new_owner)
                .unwrap()
                .clone();
            TopTradingCycle::TokenReallocation {
                newOwner: new_owner,
                tokenId: token_id,
            }
        })
        .collect()
}

fn main() {
    eprintln!("Starting guest");
    // Read the input from the guest environment.
    eprintln!("Reading input 1");
    let input: EthEvmInput = env::read();
    eprintln!("Reading input 2");
    let contract: Address = env::read();
    eprintln!("Reading input 3");
    let preferences: Vec<TopTradingCycle::TokenPreferences> = 
      <Vec<TopTradingCycle::TokenPreferences>>::abi_decode(&env::read::<Vec<u8>>(), true).unwrap();
    eprintln!("read all inputs");
    // Converts the input into a `EvmEnv` for execution. The `with_chain_spec` method is used
    // to specify the chain configuration. It checks that the state matches the state root in the
    // header provided in the input.
    let env = input.into_env().with_chain_spec(&ETH_SEPOLIA_CHAIN_SPEC);

    eprintln!("Calling contract to get preferences");
    // Execute the view call; it returns the result in the type generated by the `sol!` macro.
    let call = TopTradingCycle::getAllTokenPreferencesCall{};
    let returns = Contract::new(contract, &env).call_builder(&call).call()._0;

    eprintln!("Running the TTC solver");
    // Check that the given account holds at least 1 token.
    let reallocations: Vec<TopTradingCycle::TokenReallocation> = {
        let owner_dict = build_owner_dict(&preferences);
        reallocate(owner_dict, returns)
    };

    eprintln!("Committing the result");
    // Commit the block hash and number used when deriving `view_call_env` to the journal.
    let journal = Journal {
        commitment: env.into_commitment(),
        ttcContract: contract,
        reallocations,
    };

    eprintln!("Writing the Journal {:?}", journal);
    env::commit_slice(&journal.abi_encode());
}