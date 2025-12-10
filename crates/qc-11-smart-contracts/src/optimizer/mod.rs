//! # Bytecode Superoptimizer
//!
//! SMT-based bytecode optimization for gas efficiency.
//!
//! ## Approach
//!
//! Uses constraint-based synthesis to find gas-optimal equivalent sequences.
//!
//! ## Optimization Targets
//!
//! - Redundant PUSH/POP elimination
//! - Arithmetic pattern simplification
//! - Stack manipulation optimization
//! - Control flow optimization

pub mod patterns;
pub mod rules;
pub mod sequence;

pub use patterns::{Pattern, PatternMatcher};
pub use rules::{OptimizationRule, RuleSet};
pub use sequence::{optimize_sequence, OptimizedSequence, Instruction};
