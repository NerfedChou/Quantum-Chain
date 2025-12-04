//! # Domain Layer for Block Propagation
//!
//! Pure business logic with no I/O dependencies. This is the innermost layer
//! of the hexagonal architecture.
//!
//! ## Contents
//!
//! - **entities**: Core domain entities (`BlockAnnouncement`, `CompactBlock`, `PeerId`)
//! - **value_objects**: Configuration and state (`PropagationConfig`, `SeenBlockCache`)
//! - **services**: Domain operations (`calculate_short_id`, `reconstruct_block`)
//! - **invariants**: Security invariant checks (deduplication, rate limiting, size)
//!
//! ## Design Principles
//!
//! 1. **No I/O**: All functions are pure and synchronous
//! 2. **No External Dependencies**: Only depends on shared-types
//! 3. **Testable**: All logic can be unit tested without mocks

mod entities;
mod invariants;
mod services;
mod value_objects;

pub use entities::*;
pub use invariants::*;
pub use services::*;
pub use value_objects::*;
