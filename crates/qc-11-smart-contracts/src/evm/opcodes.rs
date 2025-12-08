//! # EVM Opcodes
//!
//! Opcode definitions and metadata for the Ethereum Virtual Machine.

/// EVM Opcode enumeration.
///
/// Complete list of EVM opcodes up to Shanghai hard fork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // 0x00 - Stop and Arithmetic
    Stop = 0x00,
    Add = 0x01,
    Mul = 0x02,
    Sub = 0x03,
    Div = 0x04,
    SDiv = 0x05,
    Mod = 0x06,
    SMod = 0x07,
    AddMod = 0x08,
    MulMod = 0x09,
    Exp = 0x0A,
    SignExtend = 0x0B,

    // 0x10 - Comparison & Bitwise
    Lt = 0x10,
    Gt = 0x11,
    SLt = 0x12,
    SGt = 0x13,
    Eq = 0x14,
    IsZero = 0x15,
    And = 0x16,
    Or = 0x17,
    Xor = 0x18,
    Not = 0x19,
    Byte = 0x1A,
    Shl = 0x1B,
    Shr = 0x1C,
    Sar = 0x1D,

    // 0x20 - Keccak256
    Keccak256 = 0x20,

    // 0x30 - Environmental Information
    Address = 0x30,
    Balance = 0x31,
    Origin = 0x32,
    Caller = 0x33,
    CallValue = 0x34,
    CallDataLoad = 0x35,
    CallDataSize = 0x36,
    CallDataCopy = 0x37,
    CodeSize = 0x38,
    CodeCopy = 0x39,
    GasPrice = 0x3A,
    ExtCodeSize = 0x3B,
    ExtCodeCopy = 0x3C,
    ReturnDataSize = 0x3D,
    ReturnDataCopy = 0x3E,
    ExtCodeHash = 0x3F,

    // 0x40 - Block Information
    BlockHash = 0x40,
    Coinbase = 0x41,
    Timestamp = 0x42,
    Number = 0x43,
    PrevRandao = 0x44, // Was DIFFICULTY
    GasLimit = 0x45,
    ChainId = 0x46,
    SelfBalance = 0x47,
    BaseFee = 0x48,

    // 0x50 - Stack, Memory, Storage
    Pop = 0x50,
    MLoad = 0x51,
    MStore = 0x52,
    MStore8 = 0x53,
    SLoad = 0x54,
    SStore = 0x55,
    Jump = 0x56,
    JumpI = 0x57,
    Pc = 0x58,
    MSize = 0x59,
    Gas = 0x5A,
    JumpDest = 0x5B,
    TLoad = 0x5C,   // EIP-1153
    TStore = 0x5D,  // EIP-1153
    MCopy = 0x5E,   // EIP-5656

    // 0x5F - Push0 (EIP-3855)
    Push0 = 0x5F,

    // 0x60-0x7F - Push
    Push1 = 0x60,
    Push2 = 0x61,
    Push3 = 0x62,
    Push4 = 0x63,
    Push5 = 0x64,
    Push6 = 0x65,
    Push7 = 0x66,
    Push8 = 0x67,
    Push9 = 0x68,
    Push10 = 0x69,
    Push11 = 0x6A,
    Push12 = 0x6B,
    Push13 = 0x6C,
    Push14 = 0x6D,
    Push15 = 0x6E,
    Push16 = 0x6F,
    Push17 = 0x70,
    Push18 = 0x71,
    Push19 = 0x72,
    Push20 = 0x73,
    Push21 = 0x74,
    Push22 = 0x75,
    Push23 = 0x76,
    Push24 = 0x77,
    Push25 = 0x78,
    Push26 = 0x79,
    Push27 = 0x7A,
    Push28 = 0x7B,
    Push29 = 0x7C,
    Push30 = 0x7D,
    Push31 = 0x7E,
    Push32 = 0x7F,

    // 0x80-0x8F - Dup
    Dup1 = 0x80,
    Dup2 = 0x81,
    Dup3 = 0x82,
    Dup4 = 0x83,
    Dup5 = 0x84,
    Dup6 = 0x85,
    Dup7 = 0x86,
    Dup8 = 0x87,
    Dup9 = 0x88,
    Dup10 = 0x89,
    Dup11 = 0x8A,
    Dup12 = 0x8B,
    Dup13 = 0x8C,
    Dup14 = 0x8D,
    Dup15 = 0x8E,
    Dup16 = 0x8F,

    // 0x90-0x9F - Swap
    Swap1 = 0x90,
    Swap2 = 0x91,
    Swap3 = 0x92,
    Swap4 = 0x93,
    Swap5 = 0x94,
    Swap6 = 0x95,
    Swap7 = 0x96,
    Swap8 = 0x97,
    Swap9 = 0x98,
    Swap10 = 0x99,
    Swap11 = 0x9A,
    Swap12 = 0x9B,
    Swap13 = 0x9C,
    Swap14 = 0x9D,
    Swap15 = 0x9E,
    Swap16 = 0x9F,

    // 0xA0-0xA4 - Log
    Log0 = 0xA0,
    Log1 = 0xA1,
    Log2 = 0xA2,
    Log3 = 0xA3,
    Log4 = 0xA4,

    // 0xF0-0xFF - System
    Create = 0xF0,
    Call = 0xF1,
    CallCode = 0xF2,
    Return = 0xF3,
    DelegateCall = 0xF4,
    Create2 = 0xF5,
    StaticCall = 0xFA,
    Revert = 0xFD,
    Invalid = 0xFE,
    SelfDestruct = 0xFF,
}

impl Opcode {
    /// Try to decode an opcode from a byte.
    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        // Use a match for known opcodes
        match byte {
            0x00 => Some(Self::Stop),
            0x01 => Some(Self::Add),
            0x02 => Some(Self::Mul),
            0x03 => Some(Self::Sub),
            0x04 => Some(Self::Div),
            0x05 => Some(Self::SDiv),
            0x06 => Some(Self::Mod),
            0x07 => Some(Self::SMod),
            0x08 => Some(Self::AddMod),
            0x09 => Some(Self::MulMod),
            0x0A => Some(Self::Exp),
            0x0B => Some(Self::SignExtend),

            0x10 => Some(Self::Lt),
            0x11 => Some(Self::Gt),
            0x12 => Some(Self::SLt),
            0x13 => Some(Self::SGt),
            0x14 => Some(Self::Eq),
            0x15 => Some(Self::IsZero),
            0x16 => Some(Self::And),
            0x17 => Some(Self::Or),
            0x18 => Some(Self::Xor),
            0x19 => Some(Self::Not),
            0x1A => Some(Self::Byte),
            0x1B => Some(Self::Shl),
            0x1C => Some(Self::Shr),
            0x1D => Some(Self::Sar),

            0x20 => Some(Self::Keccak256),

            0x30 => Some(Self::Address),
            0x31 => Some(Self::Balance),
            0x32 => Some(Self::Origin),
            0x33 => Some(Self::Caller),
            0x34 => Some(Self::CallValue),
            0x35 => Some(Self::CallDataLoad),
            0x36 => Some(Self::CallDataSize),
            0x37 => Some(Self::CallDataCopy),
            0x38 => Some(Self::CodeSize),
            0x39 => Some(Self::CodeCopy),
            0x3A => Some(Self::GasPrice),
            0x3B => Some(Self::ExtCodeSize),
            0x3C => Some(Self::ExtCodeCopy),
            0x3D => Some(Self::ReturnDataSize),
            0x3E => Some(Self::ReturnDataCopy),
            0x3F => Some(Self::ExtCodeHash),

            0x40 => Some(Self::BlockHash),
            0x41 => Some(Self::Coinbase),
            0x42 => Some(Self::Timestamp),
            0x43 => Some(Self::Number),
            0x44 => Some(Self::PrevRandao),
            0x45 => Some(Self::GasLimit),
            0x46 => Some(Self::ChainId),
            0x47 => Some(Self::SelfBalance),
            0x48 => Some(Self::BaseFee),

            0x50 => Some(Self::Pop),
            0x51 => Some(Self::MLoad),
            0x52 => Some(Self::MStore),
            0x53 => Some(Self::MStore8),
            0x54 => Some(Self::SLoad),
            0x55 => Some(Self::SStore),
            0x56 => Some(Self::Jump),
            0x57 => Some(Self::JumpI),
            0x58 => Some(Self::Pc),
            0x59 => Some(Self::MSize),
            0x5A => Some(Self::Gas),
            0x5B => Some(Self::JumpDest),
            0x5C => Some(Self::TLoad),
            0x5D => Some(Self::TStore),
            0x5E => Some(Self::MCopy),
            0x5F => Some(Self::Push0),

            0x60..=0x7F => {
                // PUSH1-PUSH32
                let idx = byte - 0x60;
                Some(match idx {
                    0 => Self::Push1,
                    1 => Self::Push2,
                    2 => Self::Push3,
                    3 => Self::Push4,
                    4 => Self::Push5,
                    5 => Self::Push6,
                    6 => Self::Push7,
                    7 => Self::Push8,
                    8 => Self::Push9,
                    9 => Self::Push10,
                    10 => Self::Push11,
                    11 => Self::Push12,
                    12 => Self::Push13,
                    13 => Self::Push14,
                    14 => Self::Push15,
                    15 => Self::Push16,
                    16 => Self::Push17,
                    17 => Self::Push18,
                    18 => Self::Push19,
                    19 => Self::Push20,
                    20 => Self::Push21,
                    21 => Self::Push22,
                    22 => Self::Push23,
                    23 => Self::Push24,
                    24 => Self::Push25,
                    25 => Self::Push26,
                    26 => Self::Push27,
                    27 => Self::Push28,
                    28 => Self::Push29,
                    29 => Self::Push30,
                    30 => Self::Push31,
                    31 => Self::Push32,
                    _ => unreachable!(),
                })
            }

            0x80..=0x8F => {
                // DUP1-DUP16
                let idx = byte - 0x80;
                Some(match idx {
                    0 => Self::Dup1,
                    1 => Self::Dup2,
                    2 => Self::Dup3,
                    3 => Self::Dup4,
                    4 => Self::Dup5,
                    5 => Self::Dup6,
                    6 => Self::Dup7,
                    7 => Self::Dup8,
                    8 => Self::Dup9,
                    9 => Self::Dup10,
                    10 => Self::Dup11,
                    11 => Self::Dup12,
                    12 => Self::Dup13,
                    13 => Self::Dup14,
                    14 => Self::Dup15,
                    15 => Self::Dup16,
                    _ => unreachable!(),
                })
            }

            0x90..=0x9F => {
                // SWAP1-SWAP16
                let idx = byte - 0x90;
                Some(match idx {
                    0 => Self::Swap1,
                    1 => Self::Swap2,
                    2 => Self::Swap3,
                    3 => Self::Swap4,
                    4 => Self::Swap5,
                    5 => Self::Swap6,
                    6 => Self::Swap7,
                    7 => Self::Swap8,
                    8 => Self::Swap9,
                    9 => Self::Swap10,
                    10 => Self::Swap11,
                    11 => Self::Swap12,
                    12 => Self::Swap13,
                    13 => Self::Swap14,
                    14 => Self::Swap15,
                    15 => Self::Swap16,
                    _ => unreachable!(),
                })
            }

            0xA0 => Some(Self::Log0),
            0xA1 => Some(Self::Log1),
            0xA2 => Some(Self::Log2),
            0xA3 => Some(Self::Log3),
            0xA4 => Some(Self::Log4),

            0xF0 => Some(Self::Create),
            0xF1 => Some(Self::Call),
            0xF2 => Some(Self::CallCode),
            0xF3 => Some(Self::Return),
            0xF4 => Some(Self::DelegateCall),
            0xF5 => Some(Self::Create2),
            0xFA => Some(Self::StaticCall),
            0xFD => Some(Self::Revert),
            0xFE => Some(Self::Invalid),
            0xFF => Some(Self::SelfDestruct),

            _ => None,
        }
    }

    /// Get the number of bytes pushed for PUSH opcodes.
    #[must_use]
    pub fn push_size(&self) -> Option<usize> {
        let byte = *self as u8;
        if (0x60..=0x7F).contains(&byte) {
            Some((byte - 0x5F) as usize)
        } else if *self == Self::Push0 {
            Some(0)
        } else {
            None
        }
    }

    /// Returns true if this opcode terminates execution.
    #[must_use]
    pub fn is_terminating(&self) -> bool {
        matches!(
            self,
            Self::Stop
                | Self::Return
                | Self::Revert
                | Self::Invalid
                | Self::SelfDestruct
        )
    }

    /// Returns true if this is a PUSH opcode.
    #[must_use]
    pub fn is_push(&self) -> bool {
        let byte = *self as u8;
        byte == 0x5F || (0x60..=0x7F).contains(&byte)
    }

    /// Returns true if this opcode modifies state.
    #[must_use]
    pub fn is_state_modifying(&self) -> bool {
        matches!(
            self,
            Self::SStore
                | Self::TStore
                | Self::Log0
                | Self::Log1
                | Self::Log2
                | Self::Log3
                | Self::Log4
                | Self::Create
                | Self::Create2
                | Self::Call
                | Self::CallCode
                | Self::DelegateCall
                | Self::SelfDestruct
        )
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_from_byte() {
        assert_eq!(Opcode::from_byte(0x00), Some(Opcode::Stop));
        assert_eq!(Opcode::from_byte(0x01), Some(Opcode::Add));
        assert_eq!(Opcode::from_byte(0x60), Some(Opcode::Push1));
        assert_eq!(Opcode::from_byte(0x7F), Some(Opcode::Push32));
        assert_eq!(Opcode::from_byte(0x80), Some(Opcode::Dup1));
        assert_eq!(Opcode::from_byte(0x90), Some(Opcode::Swap1));
        assert_eq!(Opcode::from_byte(0xF1), Some(Opcode::Call));
        assert_eq!(Opcode::from_byte(0x0C), None); // Invalid
    }

    #[test]
    fn test_push_size() {
        assert_eq!(Opcode::Push0.push_size(), Some(0));
        assert_eq!(Opcode::Push1.push_size(), Some(1));
        assert_eq!(Opcode::Push32.push_size(), Some(32));
        assert_eq!(Opcode::Add.push_size(), None);
    }

    #[test]
    fn test_is_terminating() {
        assert!(Opcode::Stop.is_terminating());
        assert!(Opcode::Return.is_terminating());
        assert!(Opcode::Revert.is_terminating());
        assert!(!Opcode::Add.is_terminating());
    }

    #[test]
    fn test_is_state_modifying() {
        assert!(Opcode::SStore.is_state_modifying());
        assert!(Opcode::Create.is_state_modifying());
        assert!(Opcode::Log0.is_state_modifying());
        assert!(!Opcode::SLoad.is_state_modifying());
        assert!(!Opcode::Add.is_state_modifying());
    }
}
