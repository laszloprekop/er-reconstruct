//! Minimal regulation slice, extracted from the reader's `util::regulation`
//! (ADR-0010). The seed needs exactly one thing from the regulation blob:
//! recognising a PlayStation *Save Wizard* export, which decrypts the blob and
//! checks its DCX compression header. The reader keeps the heavy param/BND4
//! unpacking (`params_from_regulation`, `unpack`, name tables) — that is game
//! reference data, not reconstruction, and never enters this core.

use std::io::Error;

use aes::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use binary_reader::{BinaryReader, Endian};

pub struct Regulation;

impl Regulation {
    /// True when `bytes` is an encrypted regulation blob whose decrypted payload
    /// carries a recognised DCX (ZSTD/DFLT) header — the signature of a PS Save
    /// Wizard save. Any decrypt or header mismatch reads as "not a Save Wizard
    /// save" (`Ok(false)`), never a panic.
    pub fn check_save_compression(bytes: &[u8]) -> Result<bool, Error> {
        let decrypted = Self::decrypt(bytes)?;
        Self::check_compression(&decrypted)
    }

    // Decrypt the regulation file (AES-256-CBC, IV prefixed).
    fn decrypt(cipher_text: &[u8]) -> Result<Vec<u8>, Error> {
        type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
        let key = [
            0x99, 0xBF, 0xFC, 0x36, 0x6A, 0x6B, 0xC8, 0xC6, 0xF5, 0x82, 0x7D, 0x09, 0x36, 0x02,
            0xD6, 0x76, 0xC4, 0x28, 0x92, 0xA0, 0x1C, 0x20, 0x7F, 0xB0, 0x24, 0xD3, 0xAF, 0x4E,
            0x49, 0x3F, 0xEF, 0x99,
        ];
        let iv = &cipher_text[0..16];
        let mut buf = cipher_text[16..cipher_text.len()].to_vec();
        Aes256CbcDec::new(&key.into(), iv.into())
            .decrypt_padded_mut::<NoPadding>(&mut buf)
            .map_err(|_e| Error::other("upps"))
            .map(|pt| pt.to_vec())
    }

    fn check_compression(bytes: &[u8]) -> Result<bool, Error> {
        let mut br = BinaryReader::from_u8(bytes);
        br.endian = Endian::Big;

        // Define a helper macro to reduce redundancy
        macro_rules! check {
            ($expr:expr, $expected:expr) => {
                if $expr? != $expected {
                    return Ok(false);
                }
            };
        }

        // Perform the existing checks using the helper macro
        check!(br.read_bytes(4), b"DCX\0");
        check!(br.read_i32(), 0x11000);
        check!(br.read_i32(), 0x18);
        check!(br.read_i32(), 0x24);
        check!(br.read_i32(), 0x44);
        check!(br.read_i32(), 0x4c);
        check!(br.read_bytes(4), b"DCS\0");

        // Read decompressed and compressed sizes (used later if needed)
        let _decompressed_size = br.read_i32()?;
        let _compressed_size = br.read_i32()?;

        // Check for compression type (either ZSTD or DFLT)
        check!(br.read_bytes(4), b"DCP\0");
        let compression_type = br.read_bytes(4)?;
        if compression_type != b"ZSTD" && compression_type != b"DFLT" {
            return Ok(false);
        }

        check!(br.read_i32(), 0x20);

        // Read the compression level (no specific assertion here)
        let _compression_level = br.read_u8()?;

        // Read remaining header values without strict assertions
        let _unknown1 = br.read_u8()?;
        let _unknown2 = br.read_u8()?;
        let _unknown3 = br.read_u8()?;
        let _unknown4 = br.read_i32()?;
        let _unknown5 = br.read_u8()?;
        let _unknown6 = br.read_u8()?;
        let _unknown7 = br.read_u8()?;
        let _unknown8 = br.read_u8()?;
        let _unknown9 = br.read_i32()?;
        let _unknown10 = br.read_i32()?;

        // Final checks
        check!(br.read_bytes(4), b"DCA\0");
        check!(br.read_i32(), 8);

        Ok(true)
    }
}
