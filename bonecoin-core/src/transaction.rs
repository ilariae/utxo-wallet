//! A UTXO style transaction consumes zero or more inputs and creates zero or more outputs.
//! The transaction type is the core in the transaction graph that is the history of the bonecoin economic system.
//! Every valid transaction in the history of bonecoin will be included in this graph.

use crate::{hash, Coin, CoinId, Signature};

/// A Bonecoin Transaction
///
/// In order for a bonecoin transaction to be valid:
/// * It must consume at least one input.
/// * it must consume more bones than it creates (or an equal number).
/// * Signatures must be valid.
/// 
/// The wallet does not need to check incoming transactions, but it does need to ensure that it is not creating invalid transactions for its users.
#[derive(Clone, Hash, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct Transaction {
    pub inputs: Vec<Input>,
    pub outputs: Vec<Coin>,
}

impl Transaction {
    /// Calculate the id of this transaction
    pub fn id(&self) -> TransactionId {
        TransactionId(hash(self))
    }

    /// Calculate the id of a coin created by this transaction.
    /// Since a transaction can create multiple coins, you must specify the index
    /// of the coin in this transaction and the block number in which this transaction is included.
    pub fn coin_id(&self, block_number: u64, output_index: usize) -> CoinId {
        CoinId(hash(&(&self.id(), block_number, output_index)))
    }

    /// Returns an iterator over the coin IDs that are consumed by this transaction.
    /// The input coins themselves are not available in the transaction.
    /// In most contexts (like a node or a wallet) there is an auxiliary DB that stores coins.
    pub fn iter_input_coin_ids(&self) -> impl Iterator<Item = CoinId> + '_ {
        self.inputs.iter().map(|i| i.coin_id)
    }

    /// Returns an iterator of the coins that are created by this transaction as well as their coin ids.
    pub fn iter_output_coins_and_ids(
        &self,
        block_number: u64,
    ) -> impl Iterator<Item = (CoinId, Coin)> + '_ {
        self.outputs
            .iter()
            .cloned()
            .enumerate()
            .map(move |(index, coin)| {
                let coin_id = self.coin_id(block_number, index);

                (coin_id, coin.clone())
            })
    }
}

/// Represents information about a coin to be consumed in a transaction.
/// To consume a coin, the transaction must specify which coin is being consumed and provide a valid signature from the coin's owner.
///
/// The wallet does not need to verify signatures when importing transactions; that is the blockchain's responsibility.
/// However, the wallet must provide valid signatures when creating transactions.
#[derive(Clone, Eq, Hash, PartialEq, Debug, Ord, PartialOrd)]
pub struct Input {
    /// Specifies which coin is being spent.
    pub coin_id: CoinId,
    /// A signature by the owner of this coin over the _entire transaction_ spending the coin.
    /// In this implementation, signatures are mocked, so there is no real cryptographic signing.
    pub signature: Signature,
}

impl Input {
    /// Create a dummy input for use in testing when the value does not matter.
    pub fn dummy() -> Self {
        Self {
            // Decimal digits of e as a placeholder value. This is an internal implementation detail.
            coin_id: CoinId(2718281828459045),
            signature: Signature::Invalid,
        }
    }
}

/// A unique identifier for a transaction. It is a wrapper around the hash of the transaction.
#[derive(Copy, Hash, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct TransactionId(u64);
