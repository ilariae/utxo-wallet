//! The core types associated with the Bonecoin Blockchain.
//!
//! Bonecoin is a simple UTXO based cryptocurrency.
//! A bonecoin has a value and an owner.
//! Each transaction consumes some bonecoins and creates new bonecoins.
//! The total value of the coins consumed must be less than or equal to the total value of the coins created.
//! A block has some header information, and an ordered list of transaction that move bones around.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

mod address;
mod block;
mod coin;
mod node;
mod transaction;
mod wallet;

pub use address::{Address, Signature};
pub use block::{Block, BlockId};
pub use coin::{Coin, CoinId};
pub use node::{MockNode, NodeEndpoint};
pub use transaction::{Input, Transaction, TransactionId};
pub use wallet::{WalletApi, WalletError, WalletResult};

/// Simple internal helper to do some hashing.
fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
