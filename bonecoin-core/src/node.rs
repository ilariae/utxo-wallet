//! An interface over which an off-chain tool can interact with a bonecoin node.
//! This interface is useful for tools like wallets, indexers, block explorers, etc.
//! Additionally, it includes a mock Bonecoin node useful for writing unit tests.

use crate::{Block, BlockId, Transaction};
use std::{collections::HashMap, cell::Cell};
/// Defines a common interface for a wallet to interact with a Bonecoin node.
pub trait NodeEndpoint {
    /// Query the id of of the node's best block at a given height.
    fn best_block_at_height(&self, h: u64) -> Option<BlockId>;

    /// Fetch the entire body of a block given its block id.
    fn entire_block(&self, id: &BlockId) -> Option<Block>;
}

/// A mock Bonecoin node useful for writing unit tests.
/// 
/// The mock node also tracks how many queries have been made to it in order to test 
/// wallet code performance.
pub struct MockNode {
    /// A complete database of the blocks this node knows about.
    /// This is a complete fork tree rather than a linear chain.
    /// The only validity assumption is that every block in the DB as has a parent in the DB (except genesis).
    blocks: HashMap<BlockId, Block>,
    /// The id of the block that the node currently considers best.
    /// There is no fork choice rule or anything else to update this automatically.
    /// The user must update it manually.
    best_block: BlockId,
    /// The number of times the mock node has been queried over the NodeEndpoint interface.
    /// In testing scenarios, this is useful. For example, an inefficient wallet, may re-sync
    /// from scratch every single time, and this will catch it.
    calls_so_far: Cell<u64>,
}

impl NodeEndpoint for MockNode {
    fn best_block_at_height(&self, h: u64) -> Option<BlockId> {
        // Record the call
        self.calls_so_far.set(self.calls_so_far.get() + 1);

        // Look up the best block overall to begin with
        let mut b = self.blocks.get(&self.best_block).expect("best block should be in db");

        // If the request is for a height greater than our best height, we cannot fulfill it.
        if h > b.number {
            return None;
        }

        // Start at the current best block and iterate backwards to the requested height.
        // This is not performant but it is only for testing and probably will have very short chains.
        while b.number != h {
            b = self.blocks.get(&b.parent).expect("Every block in the db also has its parent in the db.");
        }

        // Return the ID of the canonical block at the given height
        Some(b.id())
    }

    fn entire_block(&self, id: &BlockId) -> Option<Block> {
        self.blocks.get(id).cloned()
    }
}

impl MockNode {
    /// Creates a new instance of the mock node initialized to hold only the genesis block.
    pub fn new() -> Self {
        let best_block = Block::genesis().id();
        let mut blocks = HashMap::new();
        blocks.insert(best_block, Block::genesis());

        Self {
            blocks,
            best_block,
            calls_so_far: Cell::new(0),
        }
    }

    /// Add a new block to the chain built on top of the specified parent.
    /// Returns the ID of the newly built block.
    pub fn add_block(&mut self, parent_id: BlockId, body: Vec<Transaction>) -> BlockId {
        let parent_b = self
            .blocks
            .get(&parent_id)
            .expect("Cannot build child block on a block that is not known.");
        let b = Block {
            parent: parent_id,
            number: parent_b.number + 1,
            body,
        };

        let id = b.id();
        self.blocks.insert(b.id(), b);

        id
    }

    /// Sets a new block as the mock node's best.
    /// There is no longest chain rule or anything else.
    /// This method is the primary way to change the node's best block.
    pub fn set_best(&mut self, new_best: BlockId) {
        if self.blocks.contains_key(&new_best) {
            self.best_block = new_best;
        } else {
            panic!("MockNode cannot set best block to a block that is not known.");
        }
    }

    /// Adds a new block and also marks it as the best.
    pub fn add_block_as_best(&mut self, parent_id: BlockId, body: Vec<Transaction>) -> BlockId {
        let id = self.add_block(parent_id, body);
        self.set_best(id);
        id
    }

    /// Check how many times the node has been queried
    pub fn how_many_queries(&self) -> u64 {
        self.calls_so_far.get()
    }
}

#[test]
fn correct_default() {
    let node = MockNode::new();

    assert_eq!(node.best_block, Block::genesis().id());
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.entire_block(&Block::genesis().id()), Some(Block::genesis()));
}

#[test]
fn adding_block_works_and_is_not_automatically_best() {
    let mut node = MockNode::new();
    node.add_block(Block::genesis().id(), vec![]);

    assert_eq!(node.best_block, Block::genesis().id());
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), None);
}

#[test]
fn setting_best_works() {
    let mut node = MockNode::new();
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    node.set_best(b1_id);

    assert_eq!(node.best_block, b1_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
}

#[test]
fn add_block_as_best_helper_works() {
    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);

    assert_eq!(node.best_block, b1_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
}

#[test]
fn reports_correct_ancestors_even_after_reorg() {
    let mut node = MockNode::new();

    // Build an "old" chain that will eventually be orphaned off
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);

    // Ensure the correct block is reported at all generations
    assert_eq!(node.best_block, old_b3_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(old_b1_id));
    assert_eq!(node.best_block_at_height(2), Some(old_b2_id));
    assert_eq!(node.best_block_at_height(3), Some(old_b3_id));

    // Build a "new" chain that will become the best
    // In this case, I make it best at height two: shorter than the previous best.
    // This emphasizes that there is no longest chain rule.
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block(b1_id, vec![]);
    node.set_best(b2_id);

    // Make sure the correct block is reported at all generations
    assert_eq!(node.best_block, b2_id);
    assert_eq!(node.best_block_at_height(0), Some(Block::genesis().id()));
    assert_eq!(node.best_block_at_height(1), Some(b1_id));
    assert_eq!(node.best_block_at_height(2), Some(b2_id));
    assert_eq!(node.best_block_at_height(3), None);
}