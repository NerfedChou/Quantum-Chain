//! # Optimization Patterns
//!
//! Pattern matching for bytecode sequences.

/// Abstract instruction for pattern matching.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternOp {
    /// Any single instruction
    Any,
    /// Specific opcode
    Exact(u8),
    /// PUSH followed by value
    Push(Option<Vec<u8>>),
    /// Stack manipulation
    Dup(u8),
    Swap(u8),
    Pop,
    /// Arithmetic
    Add,
    Sub,
    Mul,
    /// Capture for replacement
    Capture(usize),
}

/// Pattern for matching bytecode sequences.
#[derive(Clone, Debug)]
pub struct Pattern {
    ops: Vec<PatternOp>,
    gas_cost: u64,
}

impl Pattern {
    /// Create new pattern.
    pub fn new(ops: Vec<PatternOp>, gas_cost: u64) -> Self {
        Self { ops, gas_cost }
    }

    /// Get pattern length.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Get gas cost.
    pub fn gas_cost(&self) -> u64 {
        self.gas_cost
    }
}

/// Pattern matcher for bytecode.
#[derive(Clone, Debug, Default)]
pub struct PatternMatcher {
    patterns: Vec<(Pattern, Pattern)>, // (match, replace)
}

impl PatternMatcher {
    /// Create new matcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pattern rule.
    pub fn add_rule(&mut self, match_pattern: Pattern, replace_pattern: Pattern) {
        self.patterns.push((match_pattern, replace_pattern));
    }

    /// Count rules.
    pub fn rule_count(&self) -> usize {
        self.patterns.len()
    }

    /// Estimate gas savings for a sequence.
    pub fn estimate_savings(&self, sequence_len: usize) -> u64 {
        // Simple estimate: assume 10% of patterns match
        let potential_matches = sequence_len / 10;
        self.patterns
            .iter()
            .map(|(m, r)| {
                if m.gas_cost() > r.gas_cost() {
                    m.gas_cost() - r.gas_cost()
                } else {
                    0
                }
            })
            .sum::<u64>()
            * potential_matches as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_creation() {
        let pattern = Pattern::new(vec![PatternOp::Push(None), PatternOp::Pop], 5);
        assert_eq!(pattern.len(), 2);
        assert_eq!(pattern.gas_cost(), 5);
    }

    #[test]
    fn test_matcher() {
        let mut matcher = PatternMatcher::new();

        let match_pat = Pattern::new(vec![PatternOp::Push(None), PatternOp::Pop], 5);
        let replace_pat = Pattern::new(vec![], 0);

        matcher.add_rule(match_pat, replace_pat);
        assert_eq!(matcher.rule_count(), 1);
    }
}
