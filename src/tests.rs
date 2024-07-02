//! Tests for the bonecoin wallet

use super::*;

/// Simple helper to initialize a wallet with just one account.
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

// helper functions
// fn wallet_with_alice_and_bob() -> Wallet {
//     Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
// }

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

// Track UTXOs from two transactions in a single block
#[test]
fn extra_track_two_utxo() {

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

// Reorgs with UTXOs in the chain history check

// Reorg performance tests to make sure they aren't just syncing from genesis each time.

// Memory performance test to make sure they aren't just keeping a snapshot of the entire UTXO set at every height.
