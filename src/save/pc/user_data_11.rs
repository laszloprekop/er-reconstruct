use crate::{read::read::Read, save::common::user_data_11::UserData11};
#[cfg(feature = "save-writeback")]
use crate::write::write::Write;

#[derive(Default)]
pub struct PcUserData11 {
    pub checksum: [u8; 0x10],
    pub user_data_11: UserData11,
}


impl Read for PcUserData11 {
    fn read(br: &mut binary_reader::BinaryReader) -> Result<Self, std::io::Error> {
        let mut pc_user_data_11 = PcUserData11::default();

        // Checksum
        pc_user_data_11.checksum.copy_from_slice(br.read_bytes(0x10)?);

        // Rest of the UserData11
        pc_user_data_11.user_data_11 = UserData11::read(br)?;

        Ok(pc_user_data_11)
    }
}

#[cfg(feature = "save-writeback")]
impl Write for PcUserData11 {
    fn write(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut bytes: Vec<u8> = Vec::new();
        
        // Get UserData11 bytes
        let user_data_11_bytes = self.user_data_11.write()?;

        // Calculate checksum 
        let digest = md5::compute(&user_data_11_bytes);

        // Add Checksum
        bytes.extend(digest.iter().collect::<Vec<&u8>>());

        // Add the rest of the save slot
        bytes.extend(user_data_11_bytes);

        Ok(bytes)
    }
}
