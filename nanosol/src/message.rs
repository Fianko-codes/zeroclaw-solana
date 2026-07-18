//! Legacy and v0 (without address lookup tables) message and transaction codecs.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::{
    compact_u16::{self, CompactU16Error},
    instruction::Instruction,
    pubkey::Pubkey,
};

pub const SIGNATURE_BYTES: usize = 64;
pub const MAX_STATIC_ACCOUNT_KEYS: usize = 256;
pub const MAX_TRANSACTION_BYTES: usize = 1232;
const VERSION_PREFIX_MASK: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageVersion {
    Legacy,
    V0,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MessageHeader {
    pub num_required_signatures: u8,
    pub num_readonly_signed_accounts: u8,
    pub num_readonly_unsigned_accounts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    pub account_indexes: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub version: MessageVersion,
    pub header: MessageHeader,
    pub account_keys: Vec<Pubkey>,
    pub recent_blockhash: [u8; 32],
    pub instructions: Vec<CompiledInstruction>,
}

impl Message {
    /// Compile instructions using the canonical Solana account-key ordering.
    pub fn compile(
        version: MessageVersion,
        payer: Pubkey,
        recent_blockhash: [u8; 32],
        instructions: &[Instruction],
    ) -> Result<Self, MessageError> {
        let mut metadata = BTreeMap::<Pubkey, KeyMetadata>::new();
        for instruction in instructions {
            metadata.entry(instruction.program_id).or_default();
            for account in &instruction.accounts {
                let entry = metadata.entry(account.pubkey).or_default();
                entry.is_signer |= account.is_signer;
                entry.is_writable |= account.is_writable;
            }
        }

        let payer_metadata = metadata.entry(payer).or_default();
        payer_metadata.is_signer = true;
        payer_metadata.is_writable = true;
        metadata.remove(&payer);

        let writable_signers: Vec<Pubkey> =
            std::iter::once(payer)
                .chain(metadata.iter().filter_map(|(key, value)| {
                    (value.is_signer && value.is_writable).then_some(*key)
                }))
                .collect();
        let readonly_signers: Vec<Pubkey> = metadata
            .iter()
            .filter_map(|(key, value)| (value.is_signer && !value.is_writable).then_some(*key))
            .collect();
        let writable_non_signers: Vec<Pubkey> = metadata
            .iter()
            .filter_map(|(key, value)| (!value.is_signer && value.is_writable).then_some(*key))
            .collect();
        let readonly_non_signers: Vec<Pubkey> = metadata
            .iter()
            .filter_map(|(key, value)| (!value.is_signer && !value.is_writable).then_some(*key))
            .collect();

        let required_signatures = writable_signers.len() + readonly_signers.len();
        if version == MessageVersion::Legacy
            && required_signatures >= usize::from(VERSION_PREFIX_MASK)
        {
            return Err(MessageError::LegacySignerCountTooHigh(required_signatures));
        }

        let header = MessageHeader {
            num_required_signatures: to_u8_count("required signatures", required_signatures)?,
            num_readonly_signed_accounts: to_u8_count(
                "readonly signed accounts",
                readonly_signers.len(),
            )?,
            num_readonly_unsigned_accounts: to_u8_count(
                "readonly unsigned accounts",
                readonly_non_signers.len(),
            )?,
        };

        let account_keys: Vec<Pubkey> = writable_signers
            .into_iter()
            .chain(readonly_signers)
            .chain(writable_non_signers)
            .chain(readonly_non_signers)
            .collect();
        if account_keys.len() > MAX_STATIC_ACCOUNT_KEYS {
            return Err(MessageError::TooManyAccountKeys(account_keys.len()));
        }

        let mut indexes = BTreeMap::new();
        for (index, key) in account_keys.iter().enumerate() {
            let index = u8::try_from(index).map_err(|_| MessageError::TooManyAccountKeys(index))?;
            indexes.insert(*key, index);
        }

        let mut compiled = Vec::with_capacity(instructions.len());
        for instruction in instructions {
            let program_id_index = indexes
                .get(&instruction.program_id)
                .copied()
                .ok_or(MessageError::UnknownInstructionKey(instruction.program_id))?;
            let account_indexes = instruction
                .accounts
                .iter()
                .map(|account| {
                    indexes
                        .get(&account.pubkey)
                        .copied()
                        .ok_or(MessageError::UnknownInstructionKey(account.pubkey))
                })
                .collect::<Result<Vec<_>, _>>()?;
            compiled.push(CompiledInstruction {
                program_id_index,
                account_indexes,
                data: instruction.data.clone(),
            });
        }

        let message = Self {
            version,
            header,
            account_keys,
            recent_blockhash,
            instructions: compiled,
        };
        message.validate()?;
        Ok(message)
    }

    pub fn serialize(&self) -> Result<Vec<u8>, MessageError> {
        self.validate()?;
        let mut bytes = Vec::new();
        if self.version == MessageVersion::V0 {
            bytes.push(VERSION_PREFIX_MASK);
        }
        bytes.extend_from_slice(&[
            self.header.num_required_signatures,
            self.header.num_readonly_signed_accounts,
            self.header.num_readonly_unsigned_accounts,
        ]);
        append_len(&mut bytes, self.account_keys.len())?;
        for key in &self.account_keys {
            bytes.extend_from_slice(key.as_bytes());
        }
        bytes.extend_from_slice(&self.recent_blockhash);
        append_len(&mut bytes, self.instructions.len())?;
        for instruction in &self.instructions {
            bytes.push(instruction.program_id_index);
            append_len(&mut bytes, instruction.account_indexes.len())?;
            bytes.extend_from_slice(&instruction.account_indexes);
            append_len(&mut bytes, instruction.data.len())?;
            bytes.extend_from_slice(&instruction.data);
        }
        if self.version == MessageVersion::V0 {
            // M1 deliberately supports v0 static keys only, without ALTs.
            bytes.push(0);
        }
        if bytes.len() > MAX_TRANSACTION_BYTES {
            return Err(MessageError::WireTooLarge(bytes.len()));
        }
        Ok(bytes)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self, MessageError> {
        if bytes.len() > MAX_TRANSACTION_BYTES {
            return Err(MessageError::WireTooLarge(bytes.len()));
        }
        let mut decoder = Decoder::new(bytes);
        let message = decode_message(&mut decoder)?;
        decoder.finish()?;
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> Result<(), MessageError> {
        let account_count = self.account_keys.len();
        if account_count > MAX_STATIC_ACCOUNT_KEYS {
            return Err(MessageError::TooManyAccountKeys(account_count));
        }
        let required = usize::from(self.header.num_required_signatures);
        let readonly_signed = usize::from(self.header.num_readonly_signed_accounts);
        let readonly_unsigned = usize::from(self.header.num_readonly_unsigned_accounts);
        if required > account_count {
            return Err(MessageError::InvalidHeader(
                "required signatures exceed account keys",
            ));
        }
        if readonly_signed > required {
            return Err(MessageError::InvalidHeader(
                "readonly signed accounts exceed required signatures",
            ));
        }
        if readonly_unsigned > account_count - required {
            return Err(MessageError::InvalidHeader(
                "readonly unsigned accounts exceed unsigned accounts",
            ));
        }
        if self.version == MessageVersion::Legacy && required >= usize::from(VERSION_PREFIX_MASK) {
            return Err(MessageError::LegacySignerCountTooHigh(required));
        }

        let mut unique = BTreeSet::new();
        for key in &self.account_keys {
            if !unique.insert(*key) {
                return Err(MessageError::DuplicateAccountKey(*key));
            }
        }

        for (instruction_index, instruction) in self.instructions.iter().enumerate() {
            if usize::from(instruction.program_id_index) >= account_count {
                return Err(MessageError::ProgramIndexOutOfBounds {
                    instruction: instruction_index,
                    index: instruction.program_id_index,
                    account_count,
                });
            }
            for index in &instruction.account_indexes {
                if usize::from(*index) >= account_count {
                    return Err(MessageError::AccountIndexOutOfBounds {
                        instruction: instruction_index,
                        index: *index,
                        account_count,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub signatures: Vec<[u8; SIGNATURE_BYTES]>,
    pub message: Message,
}

impl Transaction {
    pub fn new_unsigned(message: Message) -> Self {
        let signature_count = usize::from(message.header.num_required_signatures);
        Self {
            signatures: vec![[0; SIGNATURE_BYTES]; signature_count],
            message,
        }
    }

    pub fn is_unsigned(&self) -> bool {
        self.signatures
            .iter()
            .all(|signature| signature.iter().all(|byte| *byte == 0))
    }

    pub fn serialize(&self) -> Result<Vec<u8>, MessageError> {
        self.message.validate()?;
        let required = usize::from(self.message.header.num_required_signatures);
        if self.signatures.len() != required {
            return Err(MessageError::SignatureCountMismatch {
                supplied: self.signatures.len(),
                required,
            });
        }
        let mut bytes = compact_u16::encode_len(self.signatures.len())?;
        for signature in &self.signatures {
            bytes.extend_from_slice(signature);
        }
        bytes.extend_from_slice(&self.message.serialize()?);
        if bytes.len() > MAX_TRANSACTION_BYTES {
            return Err(MessageError::WireTooLarge(bytes.len()));
        }
        Ok(bytes)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self, MessageError> {
        if bytes.len() > MAX_TRANSACTION_BYTES {
            return Err(MessageError::WireTooLarge(bytes.len()));
        }
        let mut decoder = Decoder::new(bytes);
        let signature_count = decoder.read_len()?;
        if signature_count > MAX_STATIC_ACCOUNT_KEYS {
            return Err(MessageError::TooManySignatures(signature_count));
        }
        let mut signatures = Vec::with_capacity(signature_count);
        for _ in 0..signature_count {
            signatures.push(decoder.read_array::<SIGNATURE_BYTES>()?);
        }
        let message = decode_message(&mut decoder)?;
        decoder.finish()?;
        message.validate()?;
        let required = usize::from(message.header.num_required_signatures);
        if signature_count != required {
            return Err(MessageError::SignatureCountMismatch {
                supplied: signature_count,
                required,
            });
        }
        Ok(Self {
            signatures,
            message,
        })
    }

    pub fn to_base64(&self) -> Result<String, MessageError> {
        Ok(STANDARD.encode(self.serialize()?))
    }

    pub fn from_base64(encoded: &str) -> Result<Self, MessageError> {
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| MessageError::InvalidBase64(error.to_string()))?;
        Self::deserialize(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageError {
    CompactU16(CompactU16Error),
    CountOverflow {
        field: &'static str,
        count: usize,
    },
    TooManyAccountKeys(usize),
    TooManySignatures(usize),
    LegacySignerCountTooHigh(usize),
    UnknownInstructionKey(Pubkey),
    UnknownMessageVersion(u8),
    AddressTableLookupsUnsupported(usize),
    UnexpectedEof {
        offset: usize,
        needed: usize,
    },
    TrailingBytes(usize),
    InvalidHeader(&'static str),
    DuplicateAccountKey(Pubkey),
    ProgramIndexOutOfBounds {
        instruction: usize,
        index: u8,
        account_count: usize,
    },
    AccountIndexOutOfBounds {
        instruction: usize,
        index: u8,
        account_count: usize,
    },
    SignatureCountMismatch {
        supplied: usize,
        required: usize,
    },
    WireTooLarge(usize),
    InvalidBase64(String),
}

impl fmt::Display for MessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompactU16(error) => error.fmt(formatter),
            Self::CountOverflow { field, count } => {
                write!(formatter, "{field} count {count} does not fit in the message format")
            }
            Self::TooManyAccountKeys(count) => write!(
                formatter,
                "message has {count} account keys; maximum is {MAX_STATIC_ACCOUNT_KEYS}"
            ),
            Self::TooManySignatures(count) => {
                write!(formatter, "transaction declares {count} signatures")
            }
            Self::LegacySignerCountTooHigh(count) => write!(
                formatter,
                "legacy message has {count} signers; counts of 128 or more collide with the version prefix"
            ),
            Self::UnknownInstructionKey(key) => {
                write!(formatter, "instruction references unknown key {key}")
            }
            Self::UnknownMessageVersion(version) => {
                write!(formatter, "unsupported versioned-message version {version}")
            }
            Self::AddressTableLookupsUnsupported(count) => write!(
                formatter,
                "v0 message contains {count} address-table lookups; M1 supports none"
            ),
            Self::UnexpectedEof { offset, needed } => write!(
                formatter,
                "message ended at byte {offset}; needed {needed} more bytes"
            ),
            Self::TrailingBytes(count) => write!(formatter, "message has {count} trailing bytes"),
            Self::InvalidHeader(reason) => write!(formatter, "invalid message header: {reason}"),
            Self::DuplicateAccountKey(key) => write!(formatter, "duplicate account key {key}"),
            Self::ProgramIndexOutOfBounds {
                instruction,
                index,
                account_count,
            } => write!(
                formatter,
                "instruction {instruction} program index {index} exceeds {account_count} account keys"
            ),
            Self::AccountIndexOutOfBounds {
                instruction,
                index,
                account_count,
            } => write!(
                formatter,
                "instruction {instruction} account index {index} exceeds {account_count} account keys"
            ),
            Self::SignatureCountMismatch { supplied, required } => write!(
                formatter,
                "transaction has {supplied} signatures; message requires {required}"
            ),
            Self::WireTooLarge(bytes) => write!(
                formatter,
                "serialized message or transaction is {bytes} bytes; packet maximum is {MAX_TRANSACTION_BYTES}"
            ),
            Self::InvalidBase64(error) => write!(formatter, "invalid transaction base64: {error}"),
        }
    }
}

impl std::error::Error for MessageError {}

impl From<CompactU16Error> for MessageError {
    fn from(error: CompactU16Error) -> Self {
        Self::CompactU16(error)
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct KeyMetadata {
    is_signer: bool,
    is_writable: bool,
}

fn to_u8_count(field: &'static str, count: usize) -> Result<u8, MessageError> {
    u8::try_from(count).map_err(|_| MessageError::CountOverflow { field, count })
}

fn append_len(bytes: &mut Vec<u8>, length: usize) -> Result<(), MessageError> {
    bytes.extend_from_slice(&compact_u16::encode_len(length)?);
    Ok(())
}

fn decode_message(decoder: &mut Decoder<'_>) -> Result<Message, MessageError> {
    let version = match decoder.peek_u8()? {
        byte if byte & VERSION_PREFIX_MASK == 0 => MessageVersion::Legacy,
        byte => {
            decoder.read_u8()?;
            let version = byte & !VERSION_PREFIX_MASK;
            if version != 0 {
                return Err(MessageError::UnknownMessageVersion(version));
            }
            MessageVersion::V0
        }
    };

    let header = MessageHeader {
        num_required_signatures: decoder.read_u8()?,
        num_readonly_signed_accounts: decoder.read_u8()?,
        num_readonly_unsigned_accounts: decoder.read_u8()?,
    };
    let account_count = decoder.read_len()?;
    if account_count > MAX_STATIC_ACCOUNT_KEYS {
        return Err(MessageError::TooManyAccountKeys(account_count));
    }
    let mut account_keys = Vec::with_capacity(account_count);
    for _ in 0..account_count {
        account_keys.push(Pubkey::new(decoder.read_array::<32>()?));
    }
    let recent_blockhash = decoder.read_array::<32>()?;
    let instruction_count = decoder.read_len()?;
    if instruction_count > decoder.remaining() {
        return Err(MessageError::UnexpectedEof {
            offset: decoder.position(),
            needed: instruction_count - decoder.remaining(),
        });
    }
    let mut instructions = Vec::with_capacity(instruction_count);
    for _ in 0..instruction_count {
        let program_id_index = decoder.read_u8()?;
        let account_index_count = decoder.read_len()?;
        let account_indexes = decoder.read_exact(account_index_count)?.to_vec();
        let data_length = decoder.read_len()?;
        let data = decoder.read_exact(data_length)?.to_vec();
        instructions.push(CompiledInstruction {
            program_id_index,
            account_indexes,
            data,
        });
    }
    if version == MessageVersion::V0 {
        let lookup_count = decoder.read_len()?;
        if lookup_count != 0 {
            return Err(MessageError::AddressTableLookupsUnsupported(lookup_count));
        }
    }

    Ok(Message {
        version,
        header,
        account_keys,
        recent_blockhash,
        instructions,
    })
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn position(&self) -> usize {
        self.offset
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn peek_u8(&self) -> Result<u8, MessageError> {
        self.bytes
            .get(self.offset)
            .copied()
            .ok_or(MessageError::UnexpectedEof {
                offset: self.offset,
                needed: 1,
            })
    }

    fn read_u8(&mut self) -> Result<u8, MessageError> {
        let byte = self.peek_u8()?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], MessageError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(MessageError::UnexpectedEof {
                offset: self.offset,
                needed: length,
            })?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(MessageError::UnexpectedEof {
                offset: self.offset,
                needed: length.saturating_sub(self.remaining()),
            })?;
        self.offset = end;
        Ok(value)
    }

    fn read_array<const LENGTH: usize>(&mut self) -> Result<[u8; LENGTH], MessageError> {
        let mut output = [0; LENGTH];
        output.copy_from_slice(self.read_exact(LENGTH)?);
        Ok(output)
    }

    fn read_len(&mut self) -> Result<usize, MessageError> {
        let (value, consumed) = compact_u16::decode(&self.bytes[self.offset..])?;
        self.offset += consumed;
        Ok(usize::from(value))
    }

    fn finish(self) -> Result<(), MessageError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(MessageError::TrailingBytes(self.remaining()))
        }
    }
}
