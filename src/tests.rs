//! Tests for the bonecoin wallet

use super::*;

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

// helper functions
fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}

/*fn wallet_with_multiple_users() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob, Address::Charlie].into_iter())
}*/

/// Helper to create a simple and somewhat collision unlikely transaction to mark forks.
/// When tests create forked blockchains, ensure not to accidentally create the same chain twice.
/// This marker transaction can be useful to place on the new side of the fork.
fn marker_tx() -> Transaction {
    Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 123,
            owner: Address::Custom(123),
        }],
    }
}

#[test]
fn correct_genesis_values() {
    let wallet = wallet_with_alice();

    assert_eq!(wallet.best_height(), 0);
    assert_eq!(wallet.best_hash(), Block::genesis().id());
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice).unwrap().len(), 0);
}

#[test]
fn foreign_address_error() {
    let wallet = wallet_with_alice();

    assert_eq!(
        wallet.total_assets_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
}

#[test]
fn sync_two_blocks() {
    // Build a mock node that has a simple two block chain
    let mut node = MockNode::new();
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

#[test]
fn short_reorg() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 1
    let _old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    wallet.sync(&node);

    // Reorg to longer chain of length 2
    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

//          B2 (discard)  -  B3 (discard)
//        /
//    G
//        \
//          C2            -  C3             -       C4          -        C5 (new wallet state)
#[test]
fn deep_reorg() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);
}

//                      Old_B2 (discard)   -     Old_B3 (discard)
//                  /
//              G
//                  \   B2      (should reorg the chain here)
#[test]
fn reorg_to_shorter_chain() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    // Reorg to shorter chain of length 2
    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

#[test]
fn tracks_single_utxo() {
    // We have a single transaction that consumes some made up input
    // and creates a single output to alice.
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(1, 0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([(coin_id, COIN_VALUE)]))
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
}

#[test]
fn consumes_own_utxo() {
    // All coins will be valued the same in this test
    const COIN_VALUE: u64 = 100;

    // We start by minting a coin to alice
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx_mint.coin_id(1, 0);

    // Then we burn that coin
    let input = Input {
        coin_id,
        // The signature is invalid to save syntax.
        // The wallet doesn't check validity anyway.
        // This transaction is in a block, so the wallet syncs it.
        signature: Signature::Invalid,
    };
    let tx_burn = Transaction {
        inputs: vec![input],
        outputs: vec![],
    };

    // Apply this all to a blockchain and sync the wallet.
    // We apply in two separate blocks although that shouldn't be necessary.
    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![tx_mint]);
    let _b2_id = node.add_block_as_best(b1_id, vec![tx_burn]);
    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    // Make sure the UTXO is consumed
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(HashSet::new()));
    // Pedagogy: It is reasonable that the wallet could provide details about
    // the coin even after it was spent. But requiring that gives away the trick of
    // tracking spent coins so you can revert them later.
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));
}

// Track UTXOs from two transactions in a single block check

// Track UTXOs to multiple users check

// Create manual transaction
// ... with missing input
// ... with too much output
// ... with zero output value

// Create automatic transactions
// ... with too much output
// ... with zero change

// Reorgs with UTXOs in the chain history check

// Reorg performance tests to make sure they aren't just syncing from genesis each time.

// Memory performance test to make sure they aren't just keeping a snapshot of the entire UTXO set at every height.

// bigtava tests
#[test]
fn test_reorgs_with_utxos_in_chain_history() {
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    let coin_1 = Coin {
        value: 50,
        owner: Address::Alice,
    };
    let coin_2 = Coin {
        value: 100,
        owner: Address::Alice,
    };

    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_1.clone()],
    };
    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_2.clone()],
    };

    // Old chain
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block(b1_id, vec![]);
    let b3_id = node.add_block(b2_id, vec![tx_1.clone()]);
    let old_b4_id = node.add_block(b3_id, vec![]);
    let old_b5_id = node.add_block(old_b4_id, vec![]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![tx_2.clone()]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 6);
    assert_eq!(wallet.best_hash(), old_b6_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(150));
    assert_eq!(wallet.net_worth(), 150);

    // New chain
    let new_coin = Coin {
        value: 200,
        owner: Address::Alice,
    };
    let tx_new = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![new_coin.clone()],
    };

    let new_b4_id = node.add_block_as_best(b3_id, vec![tx_new]);
    let new_b5_id = node.add_block_as_best(new_b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), new_b5_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(250));
    assert_eq!(wallet.net_worth(), 250);
}

// esteban tests

fn marker_tx_v(value: u64) -> Transaction {
    Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value,
            owner: Address::Custom(value),
        }],
    }
}

#[test]
fn reports_correct_ancestors_even_after_reorg_in_the_middle() {
    let mut node = MockNode::new();

    // Build the permanent blocks
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);

    // Now the old blcoks that will be discarted
    let old_b4_id = node.add_block_as_best(b3_id, vec![]);
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![marker_tx_v(0123)]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![marker_tx_v(456)]);

    // assert_eq!(node.best_block, old_b6_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(old_b4_id));
    assert_eq!(node.best_block_at_height(5), Some(old_b5_id));
    assert_eq!(node.best_block_at_height(6), Some(old_b6_id));
    assert_eq!(node.best_block_at_height(7), None);
    // Now build a "new" chain that will eventually become best, forked from B3
    // In this case, I make it best at height 5: shorter than the previous best.
    // This emphasizes that there is no longest chain rule.

    let b4_id = node.add_block_as_best(b3_id, vec![marker_tx_v(789)]);
    let b5_id = node.add_block_as_best(b4_id, vec![marker_tx_v(1011)]);

    // MODIFIED: commented this out
    // assert_eq!(node.best_block, b5_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(b4_id));
    assert_eq!(node.best_block_at_height(5), Some(b5_id));
    assert_eq!(node.best_block_at_height(6), None);
}

#[test]
fn reports_correct_ancestors_even_after_reorg_in_the_middle_with_atomic() {
    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]); // B1 is EMPTY

    let coin_0 = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![coin_0.clone()],
    };
    let coin_id_0 = tx_mint.coin_id(2, 0);

    let b2_id = node.add_block_as_best(b1_id, vec![tx_mint]); //B2 WITH MINT TX

    let coin_1 = Coin {
        value: 4,
        owner: Address::Bob,
    };
    let coin_2 = Coin {
        value: 6,
        owner: Address::Bob,
    };
    let coin_3 = Coin {
        value: 90,
        owner: Address::Alice,
    };
    let tx_alice_bob_0 = Transaction {
        inputs: vec![Input {
            coin_id: coin_id_0,
            signature: Signature::Invalid,
        }],
        outputs: vec![coin_1.clone(), coin_2.clone(), coin_3.clone().clone()],
    };

    let coin_id_1 = tx_alice_bob_0.coin_id(3, 0);
    let coin_id_2 = tx_alice_bob_0.coin_id(3, 1);
    let coin_id_3 = tx_alice_bob_0.coin_id(3, 2);

    let b3_id = node.add_block_as_best(b2_id, vec![tx_alice_bob_0.clone()]); // B3 WITH TXS

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), b3_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(90));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(10));
    assert_eq!(wallet.net_worth(), 100);

    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_3, 90 as u64));
    assert_eq!(
        wallet.all_coins_of(Address::Alice).unwrap(),
        expected_alice_hash_set
    );

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_1, 4 as u64));
    expected_bob_hash_set.insert((coin_id_2, 6 as u64));
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));

    assert_eq!(
        wallet.coin_details(&coin_id_0),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_1), Ok(coin_1));
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2.clone()));
    assert_eq!(wallet.coin_details(&coin_id_3), Ok(coin_3.clone()));

    // Now the old blcoks that will be discarted
    let old_b4_id = node.add_block_as_best(b3_id, vec![]); //bob 9, alice 91
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![marker_tx_v(0123)]);

    let coin_4 = Coin {
        value: 1,
        owner: Address::Alice,
    };
    let coin_5 = Coin {
        value: 3,
        owner: Address::Bob,
    };
    let tx_alice_bob_1 = Transaction {
        inputs: vec![Input {
            coin_id: coin_id_1,
            signature: Signature::Invalid,
        }],
        outputs: vec![coin_4.clone(), coin_5.clone()],
    };

    let coin_id_4 = tx_alice_bob_1.coin_id(6, 0);
    let coin_id_5 = tx_alice_bob_1.coin_id(6, 1);

    let coin_6 = Coin {
        value: 73,
        owner: Address::Alice,
    };
    let coin_7 = Coin {
        value: 20,
        owner: Address::Bob,
    };
    let tx_alice_bob_2 = Transaction {
        inputs: vec![
            Input {
                coin_id: coin_id_3,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: coin_id_5,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![coin_6.clone(), coin_7.clone()],
    };
    let coin_id_6 = tx_alice_bob_2.coin_id(6, 0);
    let coin_id_7 = tx_alice_bob_2.coin_id(6, 1);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![tx_alice_bob_1, tx_alice_bob_2]); //bob 29, alice 713
                                                                                             // assert_eq!(node.best_block, old_b6_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(old_b4_id));
    assert_eq!(node.best_block_at_height(5), Some(old_b5_id));
    assert_eq!(node.best_block_at_height(6), Some(old_b6_id));
    assert_eq!(node.best_block_at_height(7), None);

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 6);
    assert_eq!(wallet.best_hash(), old_b6_id);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(74));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(26));
    assert_eq!(wallet.net_worth(), 100);

    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_6, 73 as u64));
    expected_alice_hash_set.insert((coin_id_4, 1 as u64));
    assert_eq!(
        wallet.all_coins_of(Address::Alice).unwrap(),
        expected_alice_hash_set
    );

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_7, 20 as u64));
    expected_bob_hash_set.insert((coin_id_2, 6 as u64));
    // expected_bob_hash_set.insert((coin_id_5, 3 as u64));
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(expected_bob_hash_set));

    assert_eq!(
        wallet.coin_details(&coin_id_0),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_1),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2));
    assert_eq!(
        wallet.coin_details(&coin_id_3),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_4), Ok(coin_4));
    assert_eq!(
        wallet.coin_details(&coin_id_5),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_6), Ok(coin_6));
    assert_eq!(wallet.coin_details(&coin_id_7), Ok(coin_7));

    let coin_8 = Coin {
        value: 7,
        owner: Address::Alice,
    };
    let coin_9 = Coin {
        value: 3,
        owner: Address::Bob,
    };

    let tx_alice_bob_3 = Transaction {
        inputs: vec![
            Input {
                coin_id: coin_id_1,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: coin_id_2,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![coin_8.clone(), coin_9.clone()],
    };
    let coin_id_8 = tx_alice_bob_3.coin_id(4, 0);
    let coin_id_9 = tx_alice_bob_3.coin_id(4, 1);
    let b4_id = node.add_block_as_best(b3_id, vec![tx_alice_bob_3]); //bob 29, alice 71

    let b5_id = node.add_block_as_best(b4_id, vec![marker_tx_v(1011)]);

    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), Some(b3_id));
    assert_eq!(node.best_block_at_height(4), Some(b4_id));
    assert_eq!(node.best_block_at_height(5), Some(b5_id));
    assert_eq!(node.best_block_at_height(6), None);

    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);

    let mut expected_alice_hash_set = HashSet::new();
    expected_alice_hash_set.insert((coin_id_3, 90 as u64));
    expected_alice_hash_set.insert((coin_id_8, 7 as u64));
    assert_eq!(
        wallet.all_coins_of(Address::Alice).unwrap(),
        expected_alice_hash_set
    );

    let mut expected_bob_hash_set = HashSet::new();
    expected_bob_hash_set.insert((coin_id_9, 3 as u64));

    assert_eq!(
        wallet.coin_details(&coin_id_0),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_1),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_2),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_3), Ok(coin_3.clone()));
    assert_eq!(
        wallet.coin_details(&coin_id_4),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_5),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_6),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(
        wallet.coin_details(&coin_id_7),
        Err(WalletError::UnknownCoin)
    );
    assert_eq!(wallet.coin_details(&coin_id_8), Ok(coin_8));
    assert_eq!(wallet.coin_details(&coin_id_9), Ok(coin_9));
}

// trantorian tests

// Track UTXOs from two transactions in a single block
#[test]
fn extra_track_two_utxo() {
    // TODO: might be the easiest scenario
    const COIN_0_VALUE: u64 = 100;
    const COIN_1_VALUE: u64 = 200;
    const COIN_2_VALUE: u64 = 300;
    let coin = Coin {
        value: COIN_0_VALUE,
        owner: Address::Alice,
    };
    let coin_1 = Coin {
        value: COIN_1_VALUE,
        owner: Address::Alice,
    };
    let coin_2 = Coin {
        value: COIN_2_VALUE,
        owner: Address::Bob,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_1.clone()],
    };
    let tx_2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_2.clone()],
    };
    let coin_id = tx.coin_id(1, 0);
    let coin_id_1 = tx_1.coin_id(1, 0);
    let coin_id_2 = tx_2.coin_id(1, 0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx, tx_1, tx_2]);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(
        wallet.total_assets_of(Address::Alice),
        Ok(COIN_0_VALUE + COIN_1_VALUE)
    );
    assert_eq!(
        wallet.net_worth(),
        COIN_0_VALUE + COIN_1_VALUE + COIN_2_VALUE
    );
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([
            (coin_id, COIN_0_VALUE),
            (coin_id_1, COIN_1_VALUE)
        ]))
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
    assert_eq!(wallet.coin_details(&coin_id_1), Ok(coin_1));
    assert_eq!(wallet.coin_details(&coin_id_2), Ok(coin_2));
}

// Track UTXOs to multiple users
#[test]
fn extra_utxo_to_multiple_users() {
    // TODO: might be the easiest scenario
    const COIN_0_VALUE: u64 = 100;
    const COIN_1_VALUE: u64 = 200;
    let coin = Coin {
        value: COIN_0_VALUE,
        owner: Address::Alice,
    };
    let coin_1 = Coin {
        value: COIN_1_VALUE,
        owner: Address::Bob,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let tx_1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_1.clone()],
    };
    let coin_id = tx.coin_id(1, 0);
    let coin_id_1 = tx_1.coin_id(1, 0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx, tx_1]);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_0_VALUE));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(COIN_1_VALUE));
    assert_eq!(wallet.net_worth(), COIN_0_VALUE + COIN_1_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(HashSet::from_iter([(coin_id, COIN_0_VALUE)]))
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Ok(HashSet::from_iter([(coin_id_1, COIN_1_VALUE)]))
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
    assert_eq!(wallet.coin_details(&coin_id_1), Ok(coin_1));
}

// tommy 97 tests
#[test]
fn reorg_hard_test_hehe() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice_and_bob();
    // Mint some coins
    let coin1 = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let coin2 = Coin {
        value: 90,
        owner: Address::Alice,
    };
    let coin3 = Coin {
        value: 80,
        owner: Address::Bob,
    };
    let coin4 = Coin {
        value: 70,
        owner: Address::Bob,
    };
    let coin5 = Coin {
        value: 800,
        owner: Address::Alice,
    };

    let coin6 = Coin {
        value: 15,
        owner: Address::Alice,
    };
    let mint_tx = Transaction {
        inputs: vec![],
        outputs: vec![
            coin1.clone(),
            coin2.clone(),
            coin3.clone(),
            coin4.clone(),
            coin5.clone(),
            coin6.clone(),
        ],
    };
    let alice_100_bucks_coin = mint_tx.coin_id(1, 0);
    let alice_90_bucks_coin = mint_tx.coin_id(1, 1);
    let bob_80_bucks_coin = mint_tx.coin_id(1, 2);
    let bob_70_bucks_coin = mint_tx.coin_id(1, 3);
    let alice_800_bucks_coin = mint_tx.coin_id(1, 4);
    let alice_15_bucks_coin = mint_tx.coin_id(1, 5);
    let block_1 = node.add_block(Block::genesis().id(), vec![mint_tx]);
    let tx1 = Transaction {
        inputs: vec![Input {
            coin_id: alice_100_bucks_coin,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 50,
            owner: Address::Bob,
        }],
    };

    let bob_coin_created_at_block_2 = tx1.coin_id(2, 0);
    let block2 = node.add_block(block_1, vec![tx1]);
    let tx2_1 = Transaction {
        inputs: vec![
            Input {
                coin_id: bob_80_bucks_coin,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: alice_800_bucks_coin,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![Coin {
            value: 880,
            owner: Address::Alice,
        }],
    };
    let alice_coin_created_and_destroyed_at_block_3 = tx2_1.coin_id(3, 0);
    let tx2_2 = Transaction {
        inputs: vec![Input {
            coin_id: alice_coin_created_and_destroyed_at_block_3,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 300,
            owner: Address::Bob,
        }],
    };
    let bob_coin_created_at_block_3 = tx2_2.coin_id(3, 0);
    let block3 = node.add_block(block2, vec![tx2_1, tx2_2]);
    let tx3 = Transaction {
        inputs: vec![Input {
            coin_id: alice_15_bucks_coin,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 10,
            owner: Address::Alice,
        }],
    };
    let alice_coin_created_at_block_4 = tx3.coin_id(4, 0);
    let block_4 = node.add_block_as_best(block3, vec![tx3]);
    // Sync the wallet to a blockchain with 5 blocks
    wallet.sync(&node);
    // Check we've synched correctly
    assert_eq!(4, wallet.best_height());
    assert_eq!(block_4, wallet.best_hash());
    assert_eq!(
        Ok(HashSet::from([
            (alice_90_bucks_coin, 90),
            (alice_coin_created_at_block_4, 10)
        ])),
        wallet.all_coins_of(Address::Alice)
    );
    assert_eq!(
        Ok(HashSet::from([
            (bob_70_bucks_coin, 70),
            (bob_coin_created_at_block_2, 50),
            (bob_coin_created_at_block_3, 300)
        ])),
        wallet.all_coins_of(Address::Bob)
    );
    assert_eq!(Ok(100), wallet.total_assets_of(Address::Alice));
    assert_eq!(Ok(420), wallet.total_assets_of(Address::Bob));
    assert_eq!(520, wallet.net_worth());

    // Let's get rid of the last two blocks, to check that the created and destroyed coin at block 3 isn't in our wallet. It was created in the same block!

    node.add_block_as_best(block2, vec![marker_tx()]);
    wallet.sync(&node);
    assert!(wallet
        .coin_details(&alice_coin_created_and_destroyed_at_block_3)
        .is_err());

    // Let's reorg the last_two_blocks
    let tx2_1 = Transaction {
        inputs: vec![
            Input {
                coin_id: bob_80_bucks_coin,
                signature: Signature::Invalid,
            },
            Input {
                coin_id: alice_800_bucks_coin,
                signature: Signature::Invalid,
            },
        ],
        outputs: vec![Coin {
            value: 880,
            owner: Address::Alice,
        }],
    };

    let alice_coin_created_at_block_3 = tx2_1.coin_id(3, 0);
    let block_3 = node.add_block(block2, vec![tx2_1]);
    let tx3 = Transaction {
        inputs: vec![Input {
            coin_id: alice_90_bucks_coin,
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: 30,
            owner: Address::Alice,
        }],
    };
    let alice_coin_created_at_block_4 = tx3.coin_id(4, 0);
    let block_4 = node.add_block_as_best(block_3, vec![tx3]);

    // Sync the reorg
    wallet.sync(&node);
    assert_eq!(4, wallet.best_height());
    assert_eq!(block_4, wallet.best_hash());
    // this two are actually equal. Prior the reorg, we have spent it. Now, BOOM, 880 bucks up man
    assert_eq!(
        alice_coin_created_and_destroyed_at_block_3,
        alice_coin_created_at_block_3
    );

    assert_eq!(
        Ok(HashSet::from([
            (alice_15_bucks_coin, 15),
            (alice_coin_created_at_block_4, 30),
            (alice_coin_created_at_block_3, 880)
        ])),
        wallet.all_coins_of(Address::Alice)
    );
    assert_eq!(
        Ok(HashSet::from([
            (bob_70_bucks_coin, 70),
            (bob_coin_created_at_block_2, 50)
        ])),
        wallet.all_coins_of(Address::Bob)
    );

    assert_eq!(Ok(925), wallet.total_assets_of(Address::Alice));
    assert_eq!(Ok(120), wallet.total_assets_of(Address::Bob));
    assert_eq!(1045, wallet.net_worth());
}

//tarekkma tests

#[test]
fn total_assets_of_should_not_return_no_owned_address() {
    // https://discord.com/channels/1219966585582653471/1246066143907811368/1246112529189568555
    let wallet = Wallet::new(vec![].into_iter());

    assert_eq!(
        wallet.total_assets_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );

    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );

    // just get a coin id
    let dummy_tx = Transaction {
        inputs: vec![],
        outputs: vec![Coin {
            value: 100,
            owner: Address::Alice,
        }],
    };
    let dummy_coin = dummy_tx.coin_id(1, 0);

    assert_eq!(
        wallet.coin_details(&dummy_coin),
        Err(WalletError::UnknownCoin)
    );
}

#[test]
fn spend_utxo_in_same_block() {
    let mut node = MockNode::new();
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());

    let coin1 = Coin {
        value: 100,
        owner: Address::Alice,
    };

    let mint_tx = Transaction {
        inputs: vec![],
        outputs: vec![coin1.clone()],
    };

    let alice_100_bucks_coin = mint_tx.coin_id(1, 0);

    let tx2 = Transaction {
        inputs: vec![Input {
            coin_id: alice_100_bucks_coin,
            signature: Signature::Valid(Address::Alice),
        }],
        outputs: vec![Coin {
            value: 100,
            owner: Address::Bob,
        }],
    };

    let bob_100_bucks_coin = tx2.coin_id(1, 0);

    let tx3 = Transaction {
        inputs: vec![Input {
            coin_id: bob_100_bucks_coin,
            // invalid signature, but wallet shouldn't care
            signature: Signature::Valid(Address::Custom(223)),
        }],
        outputs: vec![Coin {
            value: 100,
            owner: Address::Custom(100),
        }],
    };

    let block_1 = node.add_block_as_best(
        Block::genesis().id(),
        vec![mint_tx.clone(), tx2.clone(), tx3],
    );
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 1);
    assert_eq!(wallet.best_hash(), block_1);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));
    assert_eq!(wallet.net_worth(), 0);

    // reorg to genesis
    node.set_best(Block::genesis().id());
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 0);
    assert_eq!(wallet.best_hash(), Block::genesis().id());
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
}

/// test sync performance with 1000 blocks
#[test]
fn perf_sync_100_blocks() {
    let mut node = MockNode::new();
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());

    let mut last_block = Block::genesis().id();
    let mut block75 = last_block;
    for i in 1..=100 {
        let tx1 = Transaction {
            inputs: vec![],
            outputs: vec![Coin {
                value: 10,
                owner: Address::Alice,
            }],
        };
        let alice_coin = tx1.coin_id(i, 0);
        let tx2 = Transaction {
            inputs: vec![Input {
                coin_id: alice_coin,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![
                Coin {
                    value: 2,
                    owner: Address::Bob,
                },
                Coin {
                    value: 3,
                    owner: Address::Alice,
                },
            ],
        };
        last_block = node.add_block_as_best(last_block, vec![tx1, tx2]);
        if i == 75 {
            block75 = last_block;
        }
    }

    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 100);
    assert_eq!(wallet.best_hash(), last_block);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(300));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(200));
    assert_eq!(wallet.net_worth(), 500);

    // reorg to genesis
    node.set_best(block75);
    wallet.sync(&node);

    println!("Queries: {}", node.how_many_queries());
    // assert!(
    //     node.how_many_queries() < (75 + 100) /* we already called 100 times at least to sync to block 100 */
    // );

    assert_eq!(wallet.best_height(), 75);
    assert_eq!(wallet.best_hash(), block75);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(225));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(150));
    assert_eq!(wallet.net_worth(), 375);
}

/// test sync performance with 100 blocks
#[test]
fn pref_sync_1000_blocks() {
    let mut node = MockNode::new();
    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());

    let mut last_block = Block::genesis().id();
    let mut block850 = last_block;
    for i in 1..=1000 {
        let tx1 = Transaction {
            inputs: vec![],
            outputs: vec![Coin {
                value: 10,
                owner: Address::Alice,
            }],
        };
        let alice_coin = tx1.coin_id(i, 0);
        let tx2 = Transaction {
            inputs: vec![Input {
                coin_id: alice_coin,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![
                Coin {
                    value: 2,
                    owner: Address::Bob,
                },
                Coin {
                    value: 3,
                    owner: Address::Alice,
                },
            ],
        };
        last_block = node.add_block_as_best(last_block, vec![tx1, tx2]);
        if i == 850 {
            block850 = last_block;
        }
    }

    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 1000);
    assert_eq!(wallet.best_hash(), last_block);
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(3000));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(2000));
    assert_eq!(wallet.net_worth(), 5000);

    // reorg to genesis
    node.set_best(block850);
    wallet.sync(&node);

    println!("Queries: {}", node.how_many_queries());
    // assert!(
    //     node.how_many_queries() < (850 + 1000) /* we already called 1000 times at least to sync to block 1000 */
    // );
}

// kwar 13

fn make_one_block_blockchain() -> (MockNode, Wallet) {
    // simple blockchain to test transaction creation

    // minting coins
    let coin_alice_1 = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let coin_alice_2 = Coin {
        value: 15,
        owner: Address::Alice,
    };
    let coin_bob_1 = Coin {
        value: 120,
        owner: Address::Bob,
    };

    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![
            coin_alice_1.clone(),
            coin_alice_2.clone(),
            coin_bob_1.clone(),
        ],
    };

    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx_mint]);

    let mut wallet = Wallet::new(vec![Address::Alice, Address::Bob].into_iter());
    wallet.sync(&node);

    (node, wallet)
}

#[test]
fn blockchain_creation() {
    let (_node, wallet) = make_one_block_blockchain();

    // MODIFIED: commented this out
    // wallet.print_utxo();
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(100 + 15));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(120));
    assert_eq!(wallet.net_worth(), 100 + 15 + 120);
}

#[test]
fn transaction_with_zero_value_fails() {
    let (_, wallet) = make_one_block_blockchain();

    // now test with manual
    let (coin_id, _) = wallet
        .all_coins_of(Address::Alice)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let result = wallet.create_manual_transaction(
        vec![coin_id],
        vec![Coin {
            value: 0,
            owner: Address::Eve,
        }],
    );
    assert_eq!(result, Err(WalletError::ZeroCoinValue));

    // now check a failing transaction to zero value outputs for both automatic and manual transactions
    let result = wallet.create_automatic_transaction(Address::Charlie, 0, 0);
    assert_eq!(result, Err(WalletError::ZeroCoinValue));
}

#[test]
fn process_new_block() {
    let (mut node, mut wallet) = make_one_block_blockchain();

    let result = wallet.create_automatic_transaction(Address::Charlie, 26, 2);
    let tx = result.unwrap();
    let b1_id = node.best_block_at_height(1).unwrap();
    node.add_block_as_best(b1_id, vec![tx]);
    wallet.sync(&node);

    // MODIFIED: commented this out
    // wallet.print_utxo();

    assert_eq!(wallet.net_worth(), (100 + 15 + 120 - 26 - 2));
}

#[test]
fn transaction_simple() {
    let (_, wallet) = make_one_block_blockchain();

    let result = wallet.create_automatic_transaction(Address::Charlie, 26, 2);
    assert!(result.is_ok());
}

#[test]
fn transaction_automatic_insufficient_funds() {
    let (_, wallet) = make_one_block_blockchain();

    // now check a failing transaction due to insufficient funds
    let result = wallet.create_automatic_transaction(Address::Charlie, wallet.net_worth() - 3, 4);
    assert_eq!(result, Err(WalletError::InsufficientFunds));
}

#[test]
fn sneak_in_no_inputs() {
    let (_, wallet) = make_one_block_blockchain();
    // now try to sneak in no inputs but get an output going
    let result = wallet.create_manual_transaction(
        vec![], // no inputs
        vec![Coin {
            value: 10,
            owner: Address::Charlie, // output to Bob
        }],
    );
    assert_eq!(result, Err(WalletError::ZeroInputs));
}

#[test]
fn transaction_with_no_change_tx() {
    let (_, wallet) = make_one_block_blockchain();

    // try to create a transaction with balance exactly equal to output + burn, there won't be a change output
    let result = wallet.create_automatic_transaction(Address::Charlie, wallet.net_worth() - 3, 3);
    // MODIFIED: changed from 2 to 1, since burned coins should not be in the output
    assert!(result.unwrap().outputs.len() == 1);
}

// krayt78 tests

// Create manual transaction
// ... with missing input
#[test]
fn check_manual_transaction_with_missing_input() {
    let wallet = wallet_with_alice();
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![],
    };

    assert_eq!(
        wallet.create_manual_transaction(vec![tx.coin_id(1, 0)], vec![coin]),
        Err(WalletError::UnknownCoin)
    );
}

// ... with owner address to not be in the wallet
#[test]
fn check_manual_transaction_with_wrong_input_addresses() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };

    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };

    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx.clone()]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    assert_eq!(
        wallet.create_manual_transaction(vec![tx.coin_id(1, 0)], vec![]),
        Err(WalletError::UnknownCoin)
    );
}
// ... with too much output
#[test]
fn check_manual_transaction_with_too_much_output() {
    let wallet = wallet_with_alice();
    let coin = Coin {
        value: 100,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone(), coin.clone()],
    };

    assert_eq!(
        wallet.create_manual_transaction(vec![tx.coin_id(1, 0)], vec![coin]),
        Err(WalletError::UnknownCoin)
    );
}
// ... with zero output value
#[test]
fn check_manual_transaction_with_zero_output_value() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(1, 0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let coin_output = Coin {
        value: 0,
        owner: Address::Alice,
    };

    let mut wallet: Wallet = wallet_with_alice();
    wallet.sync(&node);

    assert_eq!(
        wallet.create_manual_transaction(vec![coin_id], vec![coin_output]),
        Err(WalletError::ZeroCoinValue)
    );
}

// Create automatic transactions
// ... with too much output
#[test]
fn check_automatic_transaction_with_too_much_output() {
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone()],
    };

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    let transaction_auto = wallet.create_automatic_transaction(Address::Bob, COIN_VALUE + 1, 0);
    assert_eq!(transaction_auto, Err(WalletError::InsufficientFunds));
}
// ... with zero change
#[test]
fn check_automatic_transaction_with_zero_change() {
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    //minting a coin to alice
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone()],
    };

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    match wallet.create_automatic_transaction(Address::Bob, 50, 50) {
        Ok(transaction) => {
            assert_eq!(transaction.inputs.len(), 1);
            assert_eq!(transaction.outputs.len(), 1);
            assert_eq!(transaction.outputs[0].value, 50);
        }
        Err(e) => {
            panic!("Error: {:?}", e);
        }
    }
}

// Reorg performance tests to make sure they aren't just syncing from genesis each time.
#[test]
fn reorg_performance() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 10
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    let old_b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![]);
    let old_b7_id = node.add_block_as_best(old_b6_id, vec![]);
    let old_b8_id = node.add_block_as_best(old_b7_id, vec![]);
    let old_b9_id = node.add_block_as_best(old_b8_id, vec![]);
    let _old_b10_id = node.add_block_as_best(old_b9_id, vec![]);
    node.add_block_as_best(old_b9_id, vec![]);
    wallet.sync(&node);

    // Reorg to shorter chain of length 8
    let b7_bis_id = node.add_block_as_best(old_b7_id, vec![marker_tx()]);
    let b8_bis_id = node.add_block_as_best(b7_bis_id, vec![]);
    wallet.sync(&node);

    println!("Wallet best_height: {:?}", wallet.best_height());
    println!("Wallet best_hash: {:?}", wallet.best_hash());

    assert_eq!(wallet.best_height(), 9);
    assert_eq!(wallet.best_hash(), b8_bis_id);
}

#[test]
fn deep_reorg_to_short_chain() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    let old_b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let old_b5_id = node.add_block_as_best(old_b4_id, vec![]);
    let old_b6_id = node.add_block_as_best(old_b5_id, vec![]);
    let _old_b7_id = node.add_block_as_best(old_b6_id, vec![]);
    wallet.sync(&node);

    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 4);
    assert_eq!(wallet.best_hash(), b4_id);
}

#[test]
fn dont_save_coins_not_owned_by_our_wallet_addresses() {
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let coin2 = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };
    let coin3 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone(), coin2.clone()],
    };

    let input = Input {
        coin_id: tx.coin_id(1, 1),
        signature: Signature::Invalid,
    };
    let tx2 = Transaction {
        inputs: vec![input],
        outputs: vec![coin3],
    };

    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![tx]);
    wallet.sync(&node);

    assert!(wallet.net_worth() == 100);

    node.add_block_as_best(b1_id, vec![tx2]);
    wallet.sync(&node);

    assert!(wallet.total_assets_of(Address::Alice) == Ok(200));
    assert!(wallet.net_worth() == 200);
}
