//! A common interface to be implemented by wallet providers.
//! Bonecoin core does not contain a wallet implementation, but it provides
//! this interface to provide a common interface to downstream wallet implementors

use std::collections::HashSet;

use crate::{Address, BlockId, Coin, CoinId, NodeEndpoint, Transaction};

/// A common interface to be implemented by wallet providers.
pub trait WalletApi {
    /// Create a new instance of the wallet that owns the given addresses
    fn new(addresses: impl Iterator<Item = Address>) -> Self;

    /// Get the height of the best block that the wallet is aware of.
    fn best_height(&self) -> u64;

    /// Get the hash of the best block that the wallet is aware of.
    fn best_hash(&self) -> BlockId;

    /// Calculate the total number of bones owned by this address.
    fn total_assets_of(&self, address: Address) -> WalletResult<u64>;

    /// Calculate the total number of bones owned by all addresses in the entire wallet.
    fn net_worth(&self) -> u64;

    /// Return the set of all UTXOs owned by the given address that
    /// the wallet knows about along with their amounts.
    fn all_coins_of(&self, address: Address) -> WalletResult<HashSet<(CoinId, u64)>>;

    /// Query the owner and value of a specific coin by its CoinId (aka its hash).
    fn coin_details(&self, coin_id: &CoinId) -> WalletResult<Coin>;

    /// Construct a transaction that consumes specific inputs and creates specific outputs.
    fn create_manual_transaction(
        &self,
        input_coin_ids: Vec<CoinId>,
        output_coins: Vec<Coin>,
    ) -> WalletResult<Transaction>;

    /// Construct a transaction that automatically selects inputs from the local database, sends the specified amount
    /// to the specified destination, burns the requested tip amount, and sends the remaining amount back to an address owned by this wallet.
    /// 
    /// There is no specific UTXO selection strategy. Wallet implementers are free to select UTXOs, ordering, etc as they want.
    /// As long as the transaction is valid and meets the requirements of the caller, this API is satisfied.
    fn create_automatic_transaction(
        &self,
        recipient: Address,
        payment_amount: u64,
        burn_aka_tip: u64,
    ) -> WalletResult<Transaction>;

    /// Synchronizes the wallet with the node. The wallet fully trusts the node and does not verify the information provided by the node.
    ///
    /// The node may occasionally experience a blockchain re-organization. When this happens, the wallet
    /// needs to detect it and update its own local database accordingly.
    fn sync<Node: NodeEndpoint>(&mut self, node: &Node);
}

/// Various errors that can occur during wallet operations.
/// 
/// The first several can happen during querying or transaction creation.
/// The latter several can only happen during transaction creation.
#[derive(Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum WalletError {
    /// The address being queried is not tracked by this wallet.
    ForeignAddress,
    /// The specified coin is not known to this wallet.
    /// This could be because the wallet is not fully synced or the coin is not owned by this wallet's addresses.
    UnknownCoin,
    /// The wallet does not own any addresses and the requested action requires an owned address.
    NoOwnedAddresses,

    /// The number of bones required by this transaction exceeds the number of bones consumed (or available to be consumed).
    /// The wallet prevents users from constructing invalid transactions.
    InsufficientFunds,
    /// You are attempting to create a coin with zero value.
    /// The wallet will not allow the user to construct an invalid transaction.
    ZeroCoinValue,
    /// Attempting to create a transaction with zero inputs.
    /// The wallet will not allow the user to construct an invalid transaction.
    ZeroInputs,
}

/// A convenient type alias to return from fallible wallet methods.
pub type WalletResult<T> = Result<T, WalletError>;
