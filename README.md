# UTXO Wallet

This project is a simplified implementation of a UTXO-based wallet and transaction system, written in Rust.  
It was developed to explore how Bitcoin-style UTXO models work at a protocol level, including transaction creation, validation, and wallet state derivation. The focus is on understanding data flow and state transitions.

## What This Project Models

The system models the core components of a UTXO-based blockchain:

- **Coins (UTXOs):** Spendable outputs that represent value
- **Transactions:** Consume existing UTXOs and create new ones
- **Wallet:** Tracks ownership by deriving balance from unspent outputs
- **Blocks:** Group transactions and update global state
- **Node:** Maintains and applies state changes

There is no account balance stored explicitly — balances are derived from unspent outputs.

## How It Works

1. Each transaction consumes one or more existing UTXOs.
2. New UTXOs are created as transaction outputs.
3. A wallet determines its balance by scanning UTXOs it owns.
4. Blocks apply transactions and update the set of unspent outputs.
5. State changes are explicit and deterministic.

This mirrors the core mechanics of Bitcoin-style systems.

## Code Structure

- `address.rs` – Address and ownership primitives
- `coin.rs` – Representation of UTXOs
- `transaction.rs` – Transaction inputs, outputs, and validation logic
- `wallet.rs` – Wallet state derivation and UTXO tracking
- `block.rs` – Block structure and transaction grouping
- `node.rs` – Node-level state handling
- `lib.rs` – Module wiring

## Tests

The project includes a comprehensive test suite that validates correctness and behavior across scenarios:

- `simple_tests.rs` – Basic wallet and transaction flows
- `tests.rs` – Core transaction and block validation
- `adv_tests.rs` – More complex scenarios and edge cases