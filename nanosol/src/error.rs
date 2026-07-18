//! Unified error wrapper for callers that do not need module-specific errors.

use std::fmt;

use crate::{
    amount::AmountError, compact_u16::CompactU16Error, instruction::InstructionError,
    message::MessageError, pubkey::ParsePubkeyError, pubkey::PdaError,
};

#[derive(Debug)]
pub enum Error {
    Amount(AmountError),
    CompactU16(CompactU16Error),
    Pubkey(ParsePubkeyError),
    Pda(PdaError),
    Instruction(InstructionError),
    Message(MessageError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Amount(error) => error.fmt(formatter),
            Self::CompactU16(error) => error.fmt(formatter),
            Self::Pubkey(error) => error.fmt(formatter),
            Self::Pda(error) => error.fmt(formatter),
            Self::Instruction(error) => error.fmt(formatter),
            Self::Message(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}

macro_rules! impl_from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Error {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

impl_from_error!(AmountError, Amount);
impl_from_error!(CompactU16Error, CompactU16);
impl_from_error!(ParsePubkeyError, Pubkey);
impl_from_error!(PdaError, Pda);
impl_from_error!(InstructionError, Instruction);
impl_from_error!(MessageError, Message);
