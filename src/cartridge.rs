pub struct Cartridge {
  data: Vec<u8>,
}

impl Cartridge {
  pub fn load(path: &str) -> Result<Cartridge, std::io::Error> {
    let data = std::fs::read(path)?;
    Ok(Cartridge { data })
  }

  pub fn read(&self, address: u16) -> u8 {
    self.data[address as usize]
  }

  pub fn title(&self) -> String {
    let bytes = &self.data[0x0134..=0x0143];
    bytes.iter()
      .take_while(|&&b| b != 0)
      .map(|&b| b as char)
      .collect()
  }
}