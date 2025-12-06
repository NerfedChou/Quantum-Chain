//! Handler for BlockFinalizedEvent from Finality (qc-09)
//!
//! When a block is finalized, this handler updates internal state
//! and can trigger cleanup of superseded block templates.
