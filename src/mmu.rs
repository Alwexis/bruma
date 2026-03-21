use crate::cartridge::Cartridge;

/*
Este es el Memory Management Unit, que es el intermediario entre la CPU y todo loq ue existe en memoria.
La CPU jamas le pregunta directamente al cartucho "dame tal byte", ni a la ram "dame este otro byte", siempre
pasa por el MMU y la MMU sabe a quién preguntarle segun la direccion.

Es importante el MMU porque en GBC el mismo espacio de 64kb contiene cosas completamente distintas: ROM, ram, registros
de video, de sonido, sprites. En resumen MMU orquesta todo este desparrame de info.
*/
pub struct MMU {
  cartridge: Cartridge, // asignamos la struct Cartridge como type de cartridge
  ram: Vec<u8>, // vector de bytes para la RAM
  vram: Vec<u8>
}

impl MMU {
  // constructor del MMU, inicializa con el cartucho especificado y con una ram de 64kb
  pub fn new(cartridge: Cartridge) -> MMU {
    MMU {
      cartridge,
      ram: vec![0u8; 0x10000],
      vram: vec![0u8; 0x2000]
    }
  }

  // leemos la direccion indicada y retornamos bytes.
  pub fn read(&self, address: u16) -> u8 {
    match address {
      // entre 0x0000 y 0x7FFF leemos y retornamos
      0x0000..=0x7FFF => self.cartridge.read(address),
      // entre 0xC000 y 0xDFFF restamos 0xC000 y retornamos el resultado
      0xC000..=0xDFFF => self.ram[(address - 0xC000) as usize],
      0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize],
      _ => 0xFF,
    }
  }

  pub fn read_u16(&self, address: u16) -> u16 {
    let low = self.read(address);
    let high = self.read(address + 1);
    let new_address = ((high as u16) << 8) | (low as u16);
    new_address
  }

  // escribimos bytes a la direccion indicada
  pub fn write(&mut self, address: u16, value: u8) {
    match address {
      0xC000..=0xDFFF => self.ram[(address - 0xC000) as usize] = value,
      0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize] = value,
      _ => {}
    }
  }

  pub fn title(&self) -> String {
    self.cartridge.title()
  }
}