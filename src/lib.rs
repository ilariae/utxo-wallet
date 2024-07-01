//! A user wallet for the bonecoin blockchain.
//!
//! Synchronizes with a blockchain node, watches for user's coins, helps construct transactions.
//!
//! Note: Reorganization handling code is not fully working in complex cases.


use std::collections::HashSet;

use bonecoin_core::*;

/// The wallet syncs and keeps a local database of information relevant to its user's addresses.
pub struct Wallet {
    addresses: HashSet<Address>, // set of addresses owned by wallet - hashset for efficiency
    ///! Note: Coins should be stored in a map rather than a set for easier access to individual coins.
    coins: HashSet<(CoinId, Coin)>, // track coins : unspent transaction outputs belonging to wallets address
    best_block_height: u64, // track height of best block that wallet is aware of - for syncs
    best_block_hash: BlockId, // track hash of best block wallet is aware of
}

impl WalletApi for Wallet {
    fn new(addresses: impl Iterator<Item = Address>) -> Self {
        let address_set: HashSet<Address> = addresses.collect(); // convert iterator into hashset

        Wallet {
            addresses: address_set,
            coins: HashSet::<(CoinId, Coin)>::new(), // initial empty set of coins
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
            .iter()
            .filter(|(_, coin)| coin.owner == address)
            .map(|(_, coin)| coin.value)
            .sum();

        Ok(total)
    }

    fn net_worth(&self) -> u64 {
        self.coins.iter().map(|(_, coin)| coin.value).sum() // total value of all coins in the wallet regardless of the owner
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
        for (id, coin) in &self.coins {
            if id == coin_id {
                return Ok(coin.clone());
            }
        }
        Err(WalletError::UnknownCoin)
    }

    fn create_manual_transaction(
        &self,
        input_coin_ids: Vec<CoinId>,
        output_coins: Vec<Coin>,
    ) -> WalletResult<Transaction> {
        //validate inputs
        if input_coin_ids.is_empty() {
            return Err(WalletError::ZeroInputs);
        }

        if output_coins.iter().any(|coin| coin.value == 0) {
            return Err(WalletError::ZeroCoinValue);
        }

        // check if all input coin IDs exist in the wallet's database
        for coin_id in &input_coin_ids {
            if !self.coins.iter().any(|(id, _)| id == coin_id) {
                return Err(WalletError::UnknownCoin);
            }
        }

        // inputs for transactions
        let inputs: Vec<Input> = input_coin_ids
            .into_iter()
            .map(|coin_id| Input {
                coin_id,
                signature: Signature::Valid(Address::Alice),
            })
            .collect();

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
        for (coin_id, coin) in &self.coins {
            selected_coins.push((*coin_id, coin.clone()));
            total_selected += coin.value;
            if total_selected >= total_needed {
                break;
            }
        }

        if total_selected < total_needed {
            return Err(WalletError::InsufficientFunds);
        }

        // prepare inputs
        let inputs: Vec<Input> = selected_coins
            .into_iter()
            .map(|(coin_id, coin)| Input {
                coin_id,
                signature: Signature::Valid(coin.owner),
            })
            .collect();

        // prepare outputs
        let mut outputs = vec![Coin {
            value: payment_amount,
            owner: recipient,
        }];

        // add change output if there is remaining value
        if total_selected > total_needed {
            let change_value = total_selected - total_needed;
            let change_address = self.addresses.iter().next().unwrap().clone();
            outputs.push(Coin {
                value: change_value,
                owner: change_address,
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
                        self.coins.retain(|(coin_id, _)| coin_id != &input.coin_id);
                        // keeps only entries whose CoinId does not match the input.coin_id
                    }

                    // add new coins created by the transaction to the wallet's UTXO set
                    for (index, coin) in transaction.outputs.iter().enumerate() {
                        let coin_id = transaction.coin_id(block.number, index);
                        if self.addresses.contains(&coin.owner) {
                            self.coins.insert((coin_id, coin.clone()));
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

// ---------- my tests -----------
#[cfg(test)]
mod self_tests {
    use super::*;
    use std::collections::HashSet;

    // helper function for tests
    fn create_coin_id(block_number: u64, output_index: usize) -> CoinId {
        let dummy_transaction = Transaction {
            inputs: vec![],
            outputs: vec![],
        };
        dummy_transaction.coin_id(block_number, output_index)
    }
    #[test]
    fn test_wallet_initialization() {
        let addresses = vec![Address::Alice, Address::Bob, Address::Charlie];
        let addresses_clone = addresses.clone();
        let wallet = Wallet::new(addresses.into_iter());

        let expected_addresses: HashSet<Address> = addresses_clone.into_iter().collect();
        assert_eq!(wallet.addresses, expected_addresses);
        assert!(wallet.coins.is_empty());
        assert_eq!(wallet.best_block_height, 0);
        assert_eq!(wallet.best_block_hash, Block::genesis().id());
    }

    #[test]
    fn test_best() {
        // simulate updating the best height and hash
        let addresses = vec![Address::Alice, Address::Bob];
        let wallet = Wallet::new(addresses.into_iter());

        // test for best height
        assert_eq!(wallet.best_height(), 0);
        let mut wallet = wallet;
        wallet.best_block_height = 5;
        assert_eq!(wallet.best_height(), 5);

        // test for best hash
        assert_eq!(wallet.best_hash(), Block::genesis().id());
        let new_block_id = Block::genesis().id(); // existing BlockId for the test
        wallet.best_block_hash = new_block_id;
        assert_eq!(wallet.best_hash(), new_block_id);
    }

    #[test]
    fn test_total_and_net() {
        let addresses = vec![Address::Alice, Address::Bob];
        let mut wallet = Wallet::new(addresses.into_iter());

        // add some coins to the wallet
        wallet.coins.insert((
            create_coin_id(0, 1),
            Coin {
                value: 100,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 2),
            Coin {
                value: 50,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 3),
            Coin {
                value: 75,
                owner: Address::Bob,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 4),
            Coin {
                value: 25,
                owner: Address::Charlie,
            },
        ));

        // test total assets for Alice
        let total_assets_alice = wallet.total_assets_of(Address::Alice).unwrap();
        assert_eq!(total_assets_alice, 150);

        // test total assets for Bob
        let total_assets_bob = wallet.total_assets_of(Address::Bob).unwrap();
        assert_eq!(total_assets_bob, 75);

        // test total assets for Charlie (should return 0 as Charlie's address is not in the wallet)
        let result = wallet.total_assets_of(Address::Charlie);
        assert_eq!(result, Err(WalletError::ForeignAddress));

        // test net worth
        let net_worth = wallet.net_worth();
        assert_eq!(net_worth, 250);
    }

    #[test]
    fn test_all_coins_of() {
        let addresses = vec![Address::Alice, Address::Bob];
        let mut wallet = Wallet::new(addresses.into_iter());

        wallet.coins.insert((
            create_coin_id(0, 1),
            Coin {
                value: 100,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 2),
            Coin {
                value: 50,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 3),
            Coin {
                value: 75,
                owner: Address::Bob,
            },
        ));

        // add a coin for Charlie who is not part of the wallet addresses
        wallet.coins.insert((
            create_coin_id(0, 4),
            Coin {
                value: 25,
                owner: Address::Charlie,
            },
        ));

        // test all coins for Alice
        let all_coins_alice = wallet.all_coins_of(Address::Alice).unwrap();
        let expected_alice: HashSet<(CoinId, u64)> =
            vec![(create_coin_id(0, 1), 100), (create_coin_id(0, 2), 50)]
                .into_iter()
                .collect();
        assert_eq!(all_coins_alice, expected_alice);

        // test all coins for Bob
        let all_coins_bob = wallet.all_coins_of(Address::Bob).unwrap();
        let expected_bob: HashSet<(CoinId, u64)> =
            vec![(create_coin_id(0, 3), 75)].into_iter().collect();
        assert_eq!(all_coins_bob, expected_bob);

        // test all coins for Charlie (should return an error as Charlie's address is not in the wallet)
        let result = wallet.all_coins_of(Address::Charlie);
        assert_eq!(result, Err(WalletError::ForeignAddress));
    }

    #[test]
    fn test_coin_details() {
        let addresses = vec![Address::Alice, Address::Bob];
        let mut wallet = Wallet::new(addresses.into_iter());

        let coin1 = Coin {
            value: 100,
            owner: Address::Alice,
        };
        let coin2 = Coin {
            value: 50,
            owner: Address::Alice,
        };
        let coin_id1 = create_coin_id(0, 1);
        let coin_id2 = create_coin_id(0, 2);
        wallet.coins.insert((coin_id1, coin1.clone()));
        wallet.coins.insert((coin_id2, coin2.clone()));

        // test coin details for a known coin
        let coin_details1 = wallet.coin_details(&coin_id1).unwrap();
        assert_eq!(coin_details1, coin1);

        // test coin details for an unknown coin
        let unknown_coin_id = create_coin_id(0, 999);
        let result = wallet.coin_details(&unknown_coin_id);
        assert_eq!(result, Err(WalletError::UnknownCoin));
    }

    #[test]
    fn test_create_manual_transaction() {
        let addresses = vec![Address::Alice, Address::Bob];
        let mut wallet = Wallet::new(addresses.into_iter());

        wallet.coins.insert((
            create_coin_id(0, 1),
            Coin {
                value: 100,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 2),
            Coin {
                value: 50,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 3),
            Coin {
                value: 75,
                owner: Address::Bob,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 4),
            Coin {
                value: 25,
                owner: Address::Charlie,
            },
        ));

        // create a manual transaction
        let coin_ids = vec![create_coin_id(0, 1), create_coin_id(0, 2)];
        let coins = vec![
            Coin {
                value: 100,
                owner: Address::Alice,
            },
            Coin {
                value: 50,
                owner: Address::Alice,
            },
        ];

        let result = wallet.create_manual_transaction(coin_ids, coins);

        assert!(result.is_ok(), "Transaction creation failed: {:?}", result);
    }

    #[test]
    fn test_create_automatic_transaction() {
        let addresses = vec![Address::Alice, Address::Bob];
        let mut wallet = Wallet::new(addresses.into_iter());

        wallet.coins.insert((
            create_coin_id(0, 1),
            Coin {
                value: 100,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 2),
            Coin {
                value: 50,
                owner: Address::Alice,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 3),
            Coin {
                value: 75,
                owner: Address::Bob,
            },
        ));
        wallet.coins.insert((
            create_coin_id(0, 4),
            Coin {
                value: 25,
                owner: Address::Charlie,
            },
        ));

        // create automatic transaction
        let destination = Address::Bob;
        let amount = 80;
        let tip_amount = 5;

        // call the function under test
        let result = wallet.create_automatic_transaction(destination, amount, tip_amount);

        assert!(
            result.is_ok(),
            "Automatic transaction creation failed: {:?}",
            result
        );
    }
}

#[cfg(test)]
mod tests;
