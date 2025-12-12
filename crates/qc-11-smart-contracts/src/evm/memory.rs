//! # EVM Memory
//!
//! Memory management for EVM execution.
//! Memory is a byte-addressable, expandable array with gas costs for expansion.

use crate::errors::VmError;

/// Maximum memory size (16 MB per System.md).
pub const MAX_MEMORY_SIZE: usize = 16 * 1024 * 1024;

/// Word size in bytes (32 bytes = 256 bits).
pub const WORD_SIZE: usize = 32;

/// EVM memory implementation.
///
/// A byte-addressable array that expands on demand.
/// Memory expansion costs gas (quadratic).
#[derive(Clone, Debug, Default)]
pub struct Memory {
    data: Vec<u8>,
}

impl Memory {
    /// Creates a new empty memory.
    #[must_use]
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Returns the current memory size in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if memory is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the memory size in 32-byte words (rounded up).
    #[must_use]
    pub fn word_size(&self) -> usize {
        self.data.len().div_ceil(WORD_SIZE)
    }

    /// Ensures memory is at least `size` bytes, expanding if necessary.
    /// Returns the number of new words added (for gas calculation).
    ///
    /// # Errors
    ///
    /// Returns `MemoryLimitExceeded` if size exceeds maximum.
    pub fn expand(&mut self, size: usize) -> Result<usize, VmError> {
        if size <= self.data.len() {
            return Ok(0);
        }

        if size > MAX_MEMORY_SIZE {
            return Err(VmError::MemoryLimitExceeded {
                requested: size,
                max: MAX_MEMORY_SIZE,
            });
        }

        // Calculate new size (round up to word boundary)
        let new_word_size = size.div_ceil(WORD_SIZE);
        let new_byte_size = new_word_size * WORD_SIZE;
        let old_word_size = self.word_size();

        // Expand with zeros
        self.data.resize(new_byte_size, 0);

        Ok(new_word_size.saturating_sub(old_word_size))
    }

    /// Read a single byte from memory.
    ///
    /// # Errors
    ///
    /// Returns `MemoryOutOfBounds` if offset is out of bounds.
    pub fn read_byte(&self, offset: usize) -> Result<u8, VmError> {
        if offset >= self.data.len() {
            return Err(VmError::MemoryOutOfBounds { offset, size: 1 });
        }
        Ok(self.data[offset])
    }

    /// Read a 32-byte word from memory.
    /// Returns zero-padded if reading past end of allocated memory.
    #[must_use]
    pub fn read_word(&self, offset: usize) -> [u8; 32] {
        let mut result = [0u8; 32];
        let len = self.data.len();

        for (i, byte) in result.iter_mut().enumerate() {
            let pos = offset.saturating_add(i);
            if pos < len {
                *byte = self.data[pos];
            }
            // Else remains 0
        }

        result
    }

    /// Read bytes from memory into a buffer.
    /// Returns zero-padded if reading past end of allocated memory.
    #[must_use]
    pub fn read_bytes(&self, offset: usize, size: usize) -> Vec<u8> {
        let mut result = vec![0u8; size];
        let len = self.data.len();

        for (i, byte) in result.iter_mut().enumerate() {
            let pos = offset.saturating_add(i);
            if pos < len {
                *byte = self.data[pos];
            }
        }

        result
    }

    /// Write a single byte to memory.
    /// Expands memory if necessary.
    ///
    /// # Errors
    ///
    /// Returns error if expansion fails.
    pub fn write_byte(&mut self, offset: usize, value: u8) -> Result<usize, VmError> {
        let words_added = self.expand(offset + 1)?;
        self.data[offset] = value;
        Ok(words_added)
    }

    /// Write a 32-byte word to memory.
    /// Expands memory if necessary.
    ///
    /// # Errors
    ///
    /// Returns error if expansion fails.
    pub fn write_word(&mut self, offset: usize, value: &[u8; 32]) -> Result<usize, VmError> {
        let words_added = self.expand(offset + 32)?;
        self.data[offset..offset + 32].copy_from_slice(value);
        Ok(words_added)
    }

    /// Write bytes to memory.
    /// Expands memory if necessary.
    ///
    /// # Errors
    ///
    /// Returns error if expansion fails.
    pub fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<usize, VmError> {
        if data.is_empty() {
            return Ok(0);
        }
        let words_added = self.expand(offset + data.len())?;
        self.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(words_added)
    }

    /// Copy bytes within memory (MCOPY opcode - EIP-5656).
    ///
    /// # Errors
    ///
    /// Returns error if expansion fails.
    pub fn copy(&mut self, dest: usize, src: usize, size: usize) -> Result<usize, VmError> {
        if size == 0 {
            return Ok(0);
        }

        // First expand to accommodate both regions
        let max_offset = dest.max(src) + size;
        let words_added = self.expand(max_offset)?;

        // Use copy_within for overlapping regions
        if src < dest && src + size > dest {
            // Overlapping: copy backwards
            for i in (0..size).rev() {
                self.data[dest + i] = self.data[src + i];
            }
        } else {
            // Non-overlapping or src >= dest
            self.data.copy_within(src..src + size, dest);
        }

        Ok(words_added)
    }

    /// Get a reference to the underlying data.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Clear memory.
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

/// Calculate memory expansion gas cost.
///
/// Cost = (`word_size^2` / 512) + (3 * `word_size`)
#[must_use]
pub fn memory_gas_cost(word_size: usize) -> u64 {
    let word_size = word_size as u64;
    (word_size * word_size / 512) + (3 * word_size)
}

/// Calculate incremental gas cost for memory expansion.
#[must_use]
pub fn memory_expansion_cost(old_word_size: usize, new_word_size: usize) -> u64 {
    if new_word_size <= old_word_size {
        return 0;
    }
    memory_gas_cost(new_word_size) - memory_gas_cost(old_word_size)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand() {
        let mut mem = Memory::new();
        assert_eq!(mem.len(), 0);

        let words = mem.expand(10).unwrap();
        assert!(words > 0);
        assert_eq!(mem.len(), 32); // Rounded to word boundary

        let words = mem.expand(64).unwrap();
        assert!(words > 0);
        assert_eq!(mem.len(), 64);
    }

    #[test]
    fn test_read_write_byte() {
        let mut mem = Memory::new();
        mem.write_byte(10, 0x42).unwrap();
        assert_eq!(mem.read_byte(10).unwrap(), 0x42);
    }

    #[test]
    fn test_read_write_word() {
        let mut mem = Memory::new();
        let word: [u8; 32] = [0x11; 32];
        mem.write_word(0, &word).unwrap();

        let read = mem.read_word(0);
        assert_eq!(read, word);
    }

    #[test]
    fn test_read_word_zero_padding() {
        let mem = Memory::new();
        let word = mem.read_word(0);
        assert_eq!(word, [0u8; 32]); // Zero-padded
    }

    #[test]
    fn test_write_bytes() {
        let mut mem = Memory::new();
        mem.write_bytes(5, &[1, 2, 3, 4]).unwrap();

        assert_eq!(mem.read_byte(5).unwrap(), 1);
        assert_eq!(mem.read_byte(6).unwrap(), 2);
        assert_eq!(mem.read_byte(7).unwrap(), 3);
        assert_eq!(mem.read_byte(8).unwrap(), 4);
    }

    #[test]
    fn test_copy_non_overlapping() {
        let mut mem = Memory::new();
        mem.write_bytes(0, &[1, 2, 3, 4]).unwrap();
        mem.copy(10, 0, 4).unwrap();

        assert_eq!(mem.read_bytes(10, 4), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_copy_overlapping() {
        let mut mem = Memory::new();
        mem.write_bytes(0, &[1, 2, 3, 4, 5]).unwrap();
        mem.copy(2, 0, 4).unwrap();

        // [1, 2, 1, 2, 3, 4]
        assert_eq!(mem.read_bytes(0, 6), vec![1, 2, 1, 2, 3, 4]);
    }

    #[test]
    fn test_memory_gas_cost() {
        assert_eq!(memory_gas_cost(0), 0);
        assert_eq!(memory_gas_cost(1), 3); // 0 + 3
        assert_eq!(memory_gas_cost(32), 98); // 32*32/512 + 3*32 = 2 + 96
    }

    #[test]
    fn test_memory_expansion_cost() {
        let cost = memory_expansion_cost(0, 1);
        assert_eq!(cost, memory_gas_cost(1));

        let cost = memory_expansion_cost(1, 1);
        assert_eq!(cost, 0); // No expansion

        let cost = memory_expansion_cost(1, 2);
        assert_eq!(cost, memory_gas_cost(2) - memory_gas_cost(1));
    }

    #[test]
    fn test_max_memory() {
        let mut mem = Memory::new();
        let result = mem.expand(MAX_MEMORY_SIZE + 1);
        assert!(result.is_err());
    }
}
