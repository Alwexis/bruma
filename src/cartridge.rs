pub struct Cartridge {
    data: Vec<u8>,
    banco_rom: usize,
}

impl Cartridge {
    pub fn load(path: &str) -> Result<Cartridge, std::io::Error> {
        let data = std::fs::read(path)?;
        Ok(Cartridge { data, banco_rom: 1 })
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.data[address as usize],
            0x4000..=0x7FFF => {
                let offset = self.banco_rom * 0x4000 + (address as usize - 0x4000);
                if offset < self.data.len() {
                    self.data[offset]
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    pub fn write_mbc(&mut self, address: u16, value: u8) {
        match address {
            // selección de banco ROM
            0x2000..=0x3FFF => {
                self.banco_rom = (value & 0x1F) as usize;
                if self.banco_rom == 0 {
                    self.banco_rom = 1;
                }
            }
            _ => {}
        }
    }

    pub fn title(&self) -> String {
        let bytes = &self.data[0x0134..=0x0143];
        bytes
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect()
    }
}
