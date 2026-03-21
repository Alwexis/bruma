use crate::mmu::MMU;

#[derive(Debug)]
pub struct CPU {
  pub a: u8,
  pub b: u8,
  pub c: u8,
  pub d: u8,
  pub e: u8,
  pub h: u8,
  pub l: u8,
  pub f: u8,
  pub pc: u16,
  pub sp: u16
}

impl CPU {
  pub fn new() -> CPU {
    CPU { a: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0, f: 0, pc: 0x0100, sp: 0xFFFE }
  }

  pub fn step(&mut self, mmu: &mut MMU) -> u8 {
    // hacemos la operacion fetch: leer el byte en lad ireccion actual del PC (program counter)
    let opcode = mmu.read(self.pc);
    self.pc = self.pc.wrapping_add(1);

    match opcode {
      0x00 => 4, // NOP (o no operation), cuesta 4 ciclos
      0xC3 => {
        let address = mmu.read_u16(self.pc);
        self.pc = address;
        16
      },
      _ => {
        panic!("Opcode no implementado: {:#04x}", opcode)
      }
    }
  }
}