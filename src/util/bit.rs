pub mod bit {
    /// Read only by the dormant save write-back path (ADR-0009), so it is dead
    /// in the default build. Kept, not deleted: `write/` is expected to keep
    /// compiling under `--features save-writeback`.
    #[allow(dead_code)]
    pub fn set_bit(byte: u8, bit_pos: u8, value: bool) -> u8 {
        if bit_pos < 8 {
            if value {
                byte | (1 << bit_pos)
            } else {
                byte & !(1 << bit_pos)
            }
        } else {
            panic!("Bit pos out of range (0-7)");
        }
    }
}
