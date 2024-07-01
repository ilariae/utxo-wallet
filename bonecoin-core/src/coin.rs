//! The basic `Coin` and `CoinId` types that represent bonecoin notes.

use crate::Address;

/// Each coin has a value denominated in bones and an owner's public address.
/// Creating a coin with zero value is invalid, as it could be freely generated and would waste space in the blockchain's state.
/// 
/// A coin is often identified by it's CoinId. Many coins have the same amount and owner.
/// Therefore a coin's unique CoinId can only be known in the context of the transaction that created it.
#[derive(Hash, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct Coin {
    /// The value of this coin denominated in bones.
    pub value: u64,
    /// The address that owns this coin and has the authority to spend it.
    pub owner: Address,
}

/// A unique identifier for a coin, encapsulating a hash value.
/// A CoinId is cryptographically linked to the transaction that created the coin, as well its output index within that transaction.
#[derive(Copy, Hash, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct CoinId(pub(crate) u64);
