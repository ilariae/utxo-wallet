//! The main `Block` and `BlockId` types for Bonecoin.
//!
//! A block contains:
//! * A cryptographic link to its parent,
//! * A number (height) that is one greater than its parent,
//! * A body of transactions that facilitate the movement of bones within the economy.

use crate::{hash, Transaction};

/// A block in the Bonecoin blockchains.
/// Unlike traditional blockchains, there is no Header/Body separation here.
#[derive(Hash, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct Block {
    /// The parent block identifier, creating a cryptographic link within the blockchain.
    pub parent: BlockId,
    /// The height of this block in the chian. (Genesis is 0.)
    pub number: u64,
    /// The list of user transactions included in the block.
    pub body: Vec<Transaction>,
}

impl Block {
    /// Calculates the identifier of this block.
    pub fn id(&self) -> BlockId {
        BlockId(hash(self))
    }

    /// Return the genesis block.
    /// This is the only valid genesis block in Bonecoin.
    pub const fn genesis() -> Self {
        Self {
            parent: BlockId(0),
            number: 0,
            body: Vec::new(),
        }
    }
}

/// A unique identifier for a block. It is a wrapper around the hash of the block.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct BlockId(u64);
