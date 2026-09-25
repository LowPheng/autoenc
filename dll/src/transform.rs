use std::io::{Cursor, Read, Seek, SeekFrom};

use cipher::{
    Array, BlockModeDecrypt, BlockModeEncrypt, BlockSizeUser, KeyIvInit, KeySizeUser,
    block_padding::Padding as CipherPadding, typenum::Unsigned,
};
use sevenz_rust2::{ArchiveEntry, ArchiveReader, ArchiveWriter, EncoderMethod, Password};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransformError {
    #[error("file is too large")]
    FileTooLarge,

    #[error("the 7z archive does not contain any files")]
    EmptyArchive,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("7z error: {0}")]
    SevenZip(#[from] sevenz_rust2::Error),
}

type Aes = aes::Aes256;
type Encryptor = cbc::Encryptor<Aes>;
type Decryptor = cbc::Decryptor<Aes>;
type Padding = cipher::block_padding::Pkcs7;
type KeySize = <Aes as KeySizeUser>::KeySize;
type BlockSize = <Aes as BlockSizeUser>::BlockSize;

pub enum DecryptionState<R: Read + Seek> {
    Unencrypted(R, u64),
    Uncompressed(Vec<u8>),
    Encrypted(Vec<u8>),
}

pub fn decrypt<R: Read + Seek>(
    mut read: R,
    key: &[u8; KeySize::USIZE],
    compression: bool,
) -> Result<DecryptionState<R>, TransformError> {
    let len = read.seek(SeekFrom::End(0))?;
    if len == 0 || !len.is_multiple_of(BlockSize::U64) {
        return Ok(DecryptionState::Unencrypted(read, len));
    }

    // Check padding
    let mut prev_block = [0u8; BlockSize::USIZE];
    if len > BlockSize::U64 {
        read.seek_relative(-32)?;
        read.read_exact(&mut prev_block)?;
    } else {
        read.rewind()?;
    }
    let mut padding_block = Array::from([0u8; BlockSize::USIZE]);
    read.read_exact(&mut padding_block)?;

    let mut decryptor = Decryptor::new(key.into(), &prev_block.into());
    decryptor.decrypt_block(&mut padding_block);
    let Ok(unpadded) = Padding::unpad(&padding_block) else {
        return Ok(DecryptionState::Unencrypted(read, len));
    };
    let actual_len = len - (BlockSize::USIZE - unpadded.len()) as u64;
    if actual_len == 0 {
        return Ok(DecryptionState::Encrypted(vec![]));
    }

    // Check first block, as invalid data may produce valid padding data.
    let mut decryptor = Decryptor::new(key.into(), &[0u8; BlockSize::USIZE].into());
    let first_block = if len > BlockSize::U64 {
        let mut block = Array::from([0u8; BlockSize::USIZE]);
        read.rewind()?;
        read.read_exact(&mut block)?;
        decryptor.decrypt_block(&mut block);
        block
    } else {
        padding_block
    };
    let first_block = &first_block[..actual_len.min(BlockSize::U64) as usize];

    const LUAC_HEADER: &[u8] = &[0x1B, 0x4C, 0x75, 0x61, 0x51, 0x00];
    const SEVENZIP_HEADER: &[u8] = &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];
    if !first_block.starts_with(SEVENZIP_HEADER)
        && !first_block.starts_with(LUAC_HEADER)
        && str::from_utf8(first_block)
            .is_err_and(|error| !(actual_len > BlockSize::U64 && error.error_len().is_none()))
    {
        // Not a valid luac header, 7z header or UTF-8 plaintext prefix.
        return Ok(DecryptionState::Unencrypted(read, len));
    }

    // Decrypt entire data
    let buf_len = actual_len
        .try_into()
        .map_err(|_| TransformError::FileTooLarge)?;
    let mut buf = Vec::with_capacity(buf_len);
    buf.extend_from_slice(first_block);
    if len > BlockSize::U64 {
        read.take(len - BlockSize::U64 * 2).read_to_end(&mut buf)?;

        let (blocks, remainder) = Array::slice_as_chunks_mut(&mut buf);
        assert!(remainder.is_empty());
        // We have the first and last blocks decrypted.
        decryptor.decrypt_blocks(&mut blocks[1..]);

        buf.extend_from_slice(unpadded);
    }
    assert_eq!(buf.len(), buf_len);

    if !compression {
        Ok(DecryptionState::Encrypted(buf))
    } else if !buf.starts_with(SEVENZIP_HEADER) {
        Ok(DecryptionState::Uncompressed(buf))
    } else {
        let mut reader = ArchiveReader::new(Cursor::new(buf), Password::empty())?;
        let mut data = None;
        reader.for_each_entries(|entry, reader| {
            if entry.is_directory() {
                return Ok(true);
            }

            let mut buf = Vec::with_capacity(entry.size() as usize);
            reader.read_to_end(&mut buf)?;
            data = Some(buf);
            Ok(false)
        })?;
        data.map_or(Err(TransformError::EmptyArchive), |buf| {
            Ok(DecryptionState::Encrypted(buf))
        })
    }
}

pub fn encrypt<R: Read>(
    mut input: R,
    input_len: u64,
    path: &str,
    key: &[u8; KeySize::USIZE],
    compression: bool,
) -> Result<Vec<u8>, TransformError> {
    let (mut buf, len) = if compression {
        let buf = Cursor::new(Vec::new());
        let mut writer = ArchiveWriter::new(buf)?;
        // For faster compression.
        writer.set_content_methods(vec![EncoderMethod::COPY.into()]);

        writer.push_archive_entry(ArchiveEntry::new_file(path), Some(input))?;

        let buf = writer.finish()?.into_inner();
        let len = buf.len();
        (buf, len)
    } else {
        let padded_len = input_len + (BlockSize::U64 - input_len % BlockSize::U64);
        let mut buf = Vec::with_capacity(padded_len as usize);
        let len = input.read_to_end(&mut buf)?;
        (buf, len)
    };
    let padded_len = len + (BlockSize::USIZE - len % BlockSize::USIZE);
    buf.resize(padded_len, 0);

    let encryptor = Encryptor::new(key.into(), &[0u8; BlockSize::USIZE].into());
    encryptor
        .encrypt_padded::<Padding>(&mut buf, len)
        .expect("Buffer length is not padded");

    Ok(buf)
}
