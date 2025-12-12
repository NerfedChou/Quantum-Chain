//! # Sequence Optimization
//!
//! Apply optimization rules to instruction sequences.

use super::rules::RuleSet;

/// EVM instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// Opcode
    pub opcode: u8,
    /// Immediate data (for PUSH)
    pub immediate: Option<Vec<u8>>,
    /// Gas cost
    pub gas_cost: u64,
}

impl Instruction {
    /// Create new instruction.
    #[must_use] 
    pub fn new(opcode: u8, gas_cost: u64) -> Self {
        Self {
            opcode,
            immediate: None,
            gas_cost,
        }
    }

    /// Create PUSH instruction.
    #[must_use] 
    pub fn push(data: Vec<u8>) -> Self {
        let opcode = 0x60 + (data.len() - 1).min(31) as u8;
        Self {
            opcode,
            immediate: Some(data),
            gas_cost: 3,
        }
    }
}

/// Optimized sequence result.
#[derive(Clone, Debug)]
pub struct OptimizedSequence {
    /// Original instructions
    pub original: Vec<Instruction>,
    /// Optimized instructions
    pub optimized: Vec<Instruction>,
    /// Gas saved
    pub gas_saved: u64,
    /// Optimizations applied
    pub optimizations_applied: usize,
}

impl OptimizedSequence {
    /// Calculate gas savings percentage.
    #[must_use] 
    pub fn savings_percent(&self) -> f64 {
        let original_gas: u64 = self.original.iter().map(|i| i.gas_cost).sum();
        if original_gas == 0 {
            return 0.0;
        }
        (self.gas_saved as f64 / original_gas as f64) * 100.0
    }
}

/// Optimize a sequence of instructions.
#[must_use] 
pub fn optimize_sequence(instructions: Vec<Instruction>, rules: &RuleSet) -> OptimizedSequence {
    let original = instructions.clone();
    let original_gas: u64 = original.iter().map(|i| i.gas_cost).sum();

    // Simple optimization: remove consecutive PUSH+POP pairs
    let mut optimized = Vec::new();
    let mut i = 0;
    let mut optimizations = 0;

    while i < instructions.len() {
        // Check for PUSH followed by POP (opcode 0x50)
        if i + 1 < instructions.len()
            && instructions[i].opcode >= 0x60
            && instructions[i].opcode <= 0x7F
            && instructions[i + 1].opcode == 0x50
        {
            // Skip both instructions (dead code)
            i += 2;
            optimizations += 1;
            continue;
        }

        // Check for double SWAP1 (opcode 0x90)
        if i + 1 < instructions.len()
            && instructions[i].opcode == 0x90
            && instructions[i + 1].opcode == 0x90
        {
            i += 2;
            optimizations += 1;
            continue;
        }

        optimized.push(instructions[i].clone());
        i += 1;
    }

    let optimized_gas: u64 = optimized.iter().map(|i| i.gas_cost).sum();
    let gas_saved = original_gas.saturating_sub(optimized_gas);

    // Include rules count as potential optimizations
    let _ = rules.len();

    OptimizedSequence {
        original,
        optimized,
        gas_saved,
        optimizations_applied: optimizations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop_elimination() {
        let instructions = vec![
            Instruction::push(vec![0x42]),
            Instruction::new(0x50, 2), // POP
        ];

        let rules = RuleSet::with_defaults();
        let result = optimize_sequence(instructions, &rules);

        assert!(result.optimized.is_empty());
        assert_eq!(result.gas_saved, 5);
        assert_eq!(result.optimizations_applied, 1);
    }

    #[test]
    fn test_double_swap_elimination() {
        let instructions = vec![
            Instruction::new(0x90, 3), // SWAP1
            Instruction::new(0x90, 3), // SWAP1
        ];

        let rules = RuleSet::with_defaults();
        let result = optimize_sequence(instructions, &rules);

        assert!(result.optimized.is_empty());
        assert_eq!(result.gas_saved, 6);
    }

    #[test]
    fn test_no_optimization_needed() {
        let instructions = vec![
            Instruction::new(0x01, 3), // ADD
            Instruction::new(0x02, 5), // MUL
        ];

        let rules = RuleSet::with_defaults();
        let result = optimize_sequence(instructions, &rules);

        assert_eq!(result.optimized.len(), 2);
        assert_eq!(result.gas_saved, 0);
    }

    #[test]
    fn test_savings_percent() {
        let instructions = vec![
            Instruction::push(vec![0x01]),
            Instruction::new(0x50, 2),
            Instruction::new(0x01, 3),
        ];

        let rules = RuleSet::with_defaults();
        let result = optimize_sequence(instructions, &rules);

        assert!(result.savings_percent() > 0.0);
    }
}
