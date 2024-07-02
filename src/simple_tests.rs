use super::*;

/// Helper functions as in the other tests.rs file 
/// 
fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}

fn marker_tx() -> Transaction {
    Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 123,
            owner: Address::Custom(123),
        }],
    }
}



// test_wallet_initialization
#[test]
fn test_wallet_initialization() {
    let wallet = wallet_with_alice();
    assert_eq!(wallet.coins.len(), 0);
    assert_eq!(wallet.addresses.len(), 1);
    assert_eq!(wallet.addresses.iter().next().unwrap(), &Address::Alice);
}

// test_best
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

// test_total_and_net
// test_all_coins_of
// test_coin_details
// test_create_manual_transaction
// test_create_automatic_transaction

