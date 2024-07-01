//! This module includes mock implementations of cryptographic primitives.

/// Represents a simulated cryptographic signature.
#[derive(Clone, Eq, Hash, PartialEq, Debug, Ord, PartialOrd)]
pub enum Signature {
    /// Represents a valid signature associated with a specific address.
    /// The application should verify that the signature is from the correct sender, though no actual cryptographic operations are performed.
    Valid(Address),
    /// Represents an invalid signature.
    Invalid,
}

/// Represents a public identifier that can own a coin.
/// A valid signature from the corresponding address is required to spend a coin.
/// This enum includes predefined variants for common names and a custom variant for other cases.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
pub enum Address {
    Alice,
    Bob,
    Charlie,
    Dave,
    Eve,
    Custom(u64),
}
