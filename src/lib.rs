//! A user wallet for the bonecoin blockchain.
//!
//! Synchronizes with a blockchain node, watches for user's coins, helps construct transactions.
//!
//! Note: Reorganization handling code is not fully working in complex cases.


use std::collections::{HashMap, HashSet};

use bonecoin_core::*;

/// The wallet syncs and keeps a local database of information relevant to its user's addresses.
pub struct Wallet {
    addresses: HashSet<Address>, // set of addresses owned by wallet - hashset for efficiency
    coins: HashMap<CoinId, Coin>, // track coins : unspent transaction outputs belonging to wallets address - stored in a map for easier access to individual coins
    best_block_height: u64, // track height of best block that wallet is aware of - for syncs
    best_block_hash: BlockId, // track hash of best block wallet is aware of
}

impl WalletApi for Wallet {
    fn new(addresses: impl Iterator<Item = Address>) -> Self {
        let address_set: HashSet<Address> = addresses.collect(); // convert iterator into hashset

        Wallet {
            addresses: address_set,
            coins: HashMap::<CoinId, Coin>::new(), // initial empty map of coins
            best_block_height: 0,                    // initial height
            best_block_hash: Block::genesis().id(),  // initial block hash (genesis default)
        }
    }

    fn best_height(&self) -> u64 {
        self.best_block_height
    }

    fn best_hash(&self) -> BlockId {
        self.best_block_hash
    }

    fn total_assets_of(&self, address: Address) -> WalletResult<u64> {
        if !self.addresses.contains(&address) {
            // check if wallet owns the given address
            return Err(WalletError::ForeignAddress);
        }

        // filter wallet's coins by the provided address and sums their values
        let total: u64 = self
            .coins
            .values()
            .filter(|coin| coin.owner == address)
            .map(|coin| coin.value)
            .sum();

        Ok(total)
    }

    fn net_worth(&self) -> u64 {
        self.coins.values().map(|coin| coin.value).sum() // total value of all coins in the wallet regardless of the owner
    }

    fn all_coins_of(&self, address: Address) -> WalletResult<HashSet<(CoinId, u64)>> {
        // returns all coins owned by a given address
        if !self.addresses.contains(&address) {
            // check if wallet owns the given address
            return Err(WalletError::ForeignAddress);
        }

        // collect all coins owned by the given address into a HashSet
        let coins: HashSet<(CoinId, u64)> = self
            .coins
            .iter()
            .filter(|(_, coin)| coin.owner == address)
            .map(|(coin_id, coin)| (*coin_id, coin.value))
            .collect();

        Ok(coins)
    }

    fn coin_details(&self, coin_id: &CoinId) -> WalletResult<Coin> {
        // retrieves the details of the corresponding coin from the wallet's database
        // look up the coin by iterating through wallets data structures
        if let Some(coin) = self.coins.get(coin_id) {
            Ok(coin.clone())
        } else {
            Err(WalletError::UnknownCoin)
        }
    }

    fn create_manual_transaction(
        &self,
        input_coin_ids: Vec<CoinId>,
        output_coins: Vec<Coin>,
    ) -> WalletResult<Transaction> {
        // Ensure all input coins exist in the wallet
        for &coin_id in &input_coin_ids {
            if !self.coins.contains_key(&coin_id) {
                return Err(WalletError::UnknownCoin);
            }
        }

        //validate inputs
        if input_coin_ids.is_empty() {
            return Err(WalletError::ZeroInputs);
        }

        if output_coins.iter().any(|coin| coin.value == 0) {
            return Err(WalletError::ZeroCoinValue);
        }

        // Create transaction inputs from the specified coin IDs
        let inputs = input_coin_ids.into_iter().map(|coin_id| Input {
            coin_id,
            signature: Signature::Valid(self.addresses.iter().next().unwrap().clone()), // Placeholder for signature
        }).collect();

        let transaction = Transaction {
            // create transaction with provided inputs and outputs
            inputs,
            outputs: output_coins,
        };

        Ok(transaction)
    }

    fn create_automatic_transaction(
        &self,
        recipient: Address,
        payment_amount: u64,
        burn_aka_tip: u64,
    ) -> WalletResult<Transaction> {
        // validate payment amount and tip
        if payment_amount == 0 {
            return Err(WalletError::ZeroCoinValue);
        }

        let total_needed = payment_amount + burn_aka_tip; // calculate total needed amount
        let mut selected_coins: Vec<(CoinId, Coin)> = Vec::new();
        let mut total_selected: u64 = 0;

        // select coins to cover total amount needed
        for (&coin_id, coin) in &self.coins {
            if total_selected >= total_needed {
                break;
            }
            selected_coins.push((coin_id, coin.clone()));
            total_selected += coin.value;
        }

        if total_selected < total_needed {
            return Err(WalletError::InsufficientFunds);
        }

        // Prepare inputs and outputs
        let inputs = selected_coins.into_iter().map(|(coin_id, coin)| Input {
            coin_id,
            signature: Signature::Valid(coin.owner),
        }).collect::<Vec<_>>();

        let mut outputs = vec![Coin {
            value: payment_amount,
            owner: recipient
        }];

        // add change output if there is remaining value
        let change_value = total_selected - total_needed;
        if change_value > 0 {
            let change_address = self.addresses.iter().next().unwrap().clone(); // Or handle change address more appropriately
            outputs.push(Coin {
                value: change_value,
                owner: change_address
            });
        }

        let transaction = Transaction { inputs, outputs }; // create the transaction
        Ok(transaction)
    }

    fn sync<Node: NodeEndpoint>(&mut self, node: &Node) {
        // rollback if reorganization is detected
        while let Some(block_id) = node.best_block_at_height(self.best_block_height) {
            if block_id == self.best_block_hash {
                break; // block_id matches, no reorganization detected, end rollback
            }
            if self.best_block_height == 0 {
                break; // reached genesis block, end rollback
            }
            // move one step back
            self.best_block_height -= 1;
            self.best_block_hash = node
                .best_block_at_height(self.best_block_height)
                .unwrap_or(Block::genesis().id());
        }

        // clear UTXO set for resync if reorganization detected
        if self.best_block_hash
            != node
                .best_block_at_height(self.best_block_height)
                .unwrap_or(Block::genesis().id())
        {
            self.coins.clear();
            self.best_block_height = 0;
            self.best_block_hash = Block::genesis().id();
        }

        // sync forward from the detected height
        while let Some(block_id) = node.best_block_at_height(self.best_block_height + 1) {
            if let Some(block) = node.entire_block(&block_id) {
                for transaction in &block.body {
                    // process transactions in the block
                    for input in &transaction.inputs {
                        self.coins.remove(&input.coin_id); // removes entries whose CoinId matches the input.coin_id
                    }

                    // add new coins created by the transaction to the wallet's UTXO set
                    for (index, coin) in transaction.outputs.iter().enumerate() {
                        let coin_id = transaction.coin_id(block.number, index);
                        if self.addresses.contains(&coin.owner) {
                            self.coins.insert(coin_id, coin.clone());
                        }
                    }
                }

                // update the wallet's best block height and hash
                self.best_block_height = block.number;
                self.best_block_hash = block_id;
            } else {
                break; // failed to fetch block, stop sync
            }
        }
    }
}


#[cfg(test)]
mod simple_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod adv_tests;

