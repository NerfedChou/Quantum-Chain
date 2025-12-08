//! # EVM Stack
//!
//! Stack management for EVM execution.
//! Maximum 1024 elements per EVM specification.

use crate::domain::value_objects::U256;
use crate::errors::VmError;

/// Maximum stack size per EVM specification.
pub const MAX_STACK_SIZE: usize = 1024;

/// EVM stack implementation.
///
/// A LIFO stack holding 256-bit values. Maximum 1024 elements.
#[derive(Clone, Debug, Default)]
pub struct Stack {
    data: Vec<U256>,
}

impl Stack {
    /// Creates a new empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(64), // Pre-allocate for common case
        }
    }

    /// Returns the number of elements on the stack.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the stack is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Push a value onto the stack.
    ///
    /// # Errors
    ///
    /// Returns `StackOverflow` if the stack is full.
    pub fn push(&mut self, value: U256) -> Result<(), VmError> {
        if self.data.len() >= MAX_STACK_SIZE {
            return Err(VmError::StackOverflow);
        }
        self.data.push(value);
        Ok(())
    }

    /// Pop a value from the stack.
    ///
    /// # Errors
    ///
    /// Returns `StackUnderflow` if the stack is empty.
    pub fn pop(&mut self) -> Result<U256, VmError> {
        self.data.pop().ok_or(VmError::StackUnderflow)
    }

    /// Peek at the top value without removing it.
    ///
    /// # Errors
    ///
    /// Returns `StackUnderflow` if the stack is empty.
    pub fn peek(&self) -> Result<U256, VmError> {
        self.data.last().copied().ok_or(VmError::StackUnderflow)
    }

    /// Peek at a value at a given depth (0 = top).
    ///
    /// # Errors
    ///
    /// Returns `StackUnderflow` if the index is out of bounds.
    pub fn peek_at(&self, depth: usize) -> Result<U256, VmError> {
        if depth >= self.data.len() {
            return Err(VmError::StackUnderflow);
        }
        Ok(self.data[self.data.len() - 1 - depth])
    }

    /// Swap the top element with the element at depth n (1-indexed).
    /// SWAP1 swaps top with second element (n=1).
    ///
    /// # Errors
    ///
    /// Returns `StackUnderflow` if not enough elements.
    pub fn swap(&mut self, n: usize) -> Result<(), VmError> {
        if n == 0 || n >= self.data.len() {
            return Err(VmError::StackUnderflow);
        }
        let len = self.data.len();
        self.data.swap(len - 1, len - 1 - n);
        Ok(())
    }

    /// Duplicate the element at depth n (0-indexed from top) and push it.
    /// DUP1 duplicates top element (n=0).
    ///
    /// # Errors
    ///
    /// Returns `StackUnderflow` if not enough elements, `StackOverflow` if full.
    pub fn dup(&mut self, n: usize) -> Result<(), VmError> {
        if n >= self.data.len() {
            return Err(VmError::StackUnderflow);
        }
        if self.data.len() >= MAX_STACK_SIZE {
            return Err(VmError::StackOverflow);
        }
        let value = self.data[self.data.len() - 1 - n];
        self.data.push(value);
        Ok(())
    }

    /// Clear the stack.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get a reference to the underlying data for debugging.
    #[must_use]
    pub fn as_slice(&self) -> &[U256] {
        &self.data
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let mut stack = Stack::new();
        stack.push(U256::from(42)).unwrap();
        stack.push(U256::from(100)).unwrap();

        assert_eq!(stack.len(), 2);
        assert_eq!(stack.pop().unwrap(), U256::from(100));
        assert_eq!(stack.pop().unwrap(), U256::from(42));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_peek() {
        let mut stack = Stack::new();
        stack.push(U256::from(1)).unwrap();
        stack.push(U256::from(2)).unwrap();
        stack.push(U256::from(3)).unwrap();

        assert_eq!(stack.peek().unwrap(), U256::from(3));
        assert_eq!(stack.peek_at(0).unwrap(), U256::from(3));
        assert_eq!(stack.peek_at(1).unwrap(), U256::from(2));
        assert_eq!(stack.peek_at(2).unwrap(), U256::from(1));
        assert!(stack.peek_at(3).is_err());
    }

    #[test]
    fn test_swap() {
        let mut stack = Stack::new();
        stack.push(U256::from(1)).unwrap();
        stack.push(U256::from(2)).unwrap();
        stack.push(U256::from(3)).unwrap();

        // SWAP1: swap top with second
        stack.swap(1).unwrap();
        assert_eq!(stack.peek_at(0).unwrap(), U256::from(2));
        assert_eq!(stack.peek_at(1).unwrap(), U256::from(3));

        // SWAP2: swap top with third
        stack.swap(2).unwrap();
        assert_eq!(stack.peek_at(0).unwrap(), U256::from(1));
        assert_eq!(stack.peek_at(2).unwrap(), U256::from(2));
    }

    #[test]
    fn test_dup() {
        let mut stack = Stack::new();
        stack.push(U256::from(1)).unwrap();
        stack.push(U256::from(2)).unwrap();

        // DUP1: duplicate top
        stack.dup(0).unwrap();
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.peek().unwrap(), U256::from(2));

        // DUP2: duplicate second from top
        stack.dup(1).unwrap();
        assert_eq!(stack.len(), 4);
        assert_eq!(stack.peek().unwrap(), U256::from(2));
    }

    #[test]
    fn test_overflow() {
        let mut stack = Stack::new();
        for i in 0..MAX_STACK_SIZE {
            stack.push(U256::from(i)).unwrap();
        }
        assert!(stack.push(U256::from(0)).is_err());
    }

    #[test]
    fn test_underflow() {
        let mut stack = Stack::new();
        assert!(stack.pop().is_err());
        assert!(stack.peek().is_err());
    }
}
