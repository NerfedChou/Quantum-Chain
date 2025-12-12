//! # Optimization Rules
//!
//! Predefined gas optimization rules.

use super::patterns::{Pattern, PatternOp};

/// Single optimization rule.
#[derive(Clone, Debug)]
pub struct OptimizationRule {
    /// Rule name
    pub name: &'static str,
    /// Pattern to match
    pub pattern: Pattern,
    /// Replacement pattern
    pub replacement: Pattern,
    /// Description
    pub description: &'static str,
}

/// Set of optimization rules.
#[derive(Clone, Debug, Default)]
pub struct RuleSet {
    rules: Vec<OptimizationRule>,
}

impl RuleSet {
    /// Create empty rule set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with default gas optimization rules.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut set = Self::new();

        // Rule 1: PUSH + POP = nothing (dead code elimination)
        set.add(OptimizationRule {
            name: "push_pop_elimination",
            pattern: Pattern::new(vec![PatternOp::Push(None), PatternOp::Pop], 5),
            replacement: Pattern::new(vec![], 0),
            description: "Remove redundant PUSH followed by POP",
        });

        // Rule 2: DUP1 + POP = nothing
        set.add(OptimizationRule {
            name: "dup1_pop_elimination",
            pattern: Pattern::new(vec![PatternOp::Dup(1), PatternOp::Pop], 6),
            replacement: Pattern::new(vec![], 0),
            description: "Remove DUP1 followed by POP",
        });

        // Rule 3: SWAP1 SWAP1 = nothing
        set.add(OptimizationRule {
            name: "double_swap_elimination",
            pattern: Pattern::new(vec![PatternOp::Swap(1), PatternOp::Swap(1)], 6),
            replacement: Pattern::new(vec![], 0),
            description: "Remove redundant double SWAP1",
        });

        // Rule 4: x + 0 = x (identity)
        set.add(OptimizationRule {
            name: "add_zero_elimination",
            pattern: Pattern::new(vec![PatternOp::Push(Some(vec![0])), PatternOp::Add], 6),
            replacement: Pattern::new(vec![], 0),
            description: "Remove addition of zero",
        });

        // Rule 5: x * 1 = x (identity)
        set.add(OptimizationRule {
            name: "mul_one_elimination",
            pattern: Pattern::new(vec![PatternOp::Push(Some(vec![1])), PatternOp::Mul], 8),
            replacement: Pattern::new(vec![], 0),
            description: "Remove multiplication by one",
        });

        set
    }

    /// Add a rule.
    pub fn add(&mut self, rule: OptimizationRule) {
        self.rules.push(rule);
    }

    /// Get all rules.
    #[must_use]
    pub fn rules(&self) -> &[OptimizationRule] {
        &self.rules
    }

    /// Count rules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Estimate total gas savings.
    #[must_use]
    pub fn estimate_max_savings(&self) -> u64 {
        self.rules
            .iter()
            .map(|r| {
                r.pattern
                    .gas_cost()
                    .saturating_sub(r.replacement.gas_cost())
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rules() {
        let rules = RuleSet::with_defaults();
        assert_eq!(rules.len(), 5);
    }

    #[test]
    fn test_gas_savings() {
        let rules = RuleSet::with_defaults();
        let savings = rules.estimate_max_savings();
        assert!(savings > 0);
    }

    #[test]
    fn test_add_custom_rule() {
        let mut rules = RuleSet::new();
        rules.add(OptimizationRule {
            name: "custom",
            pattern: Pattern::new(vec![PatternOp::Pop], 2),
            replacement: Pattern::new(vec![], 0),
            description: "Custom rule",
        });
        assert_eq!(rules.len(), 1);
    }
}
