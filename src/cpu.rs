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

  // comparamos si la posicion 7 del bit es 1
  fn get_flag_z(&self) -> bool {
    self.f & 0b10000000 != 0
  }

  // seteamos el bit 7 dependiendo del valor; si es true lo "prendemos"; si es false lo apagamos. |= y &= es asignación con OR y AND.
  // solo se activa cuando el resultado es 0.
  fn set_flag_z(&mut self, value: bool) {
    if value {
      self.f |= 0b10000000;
    } else {
      self.f &= !0b10000000;
    }
  }

  pub fn step(&mut self, mmu: &mut MMU) -> u8 {
    // hacemos la operacion fetch: leer el byte en lad ireccion actual del PC (program counter)
    let opcode = mmu.read(self.pc);
    self.pc = self.pc.wrapping_add(1);

    match opcode {
      0x00 => 4, // NOP (o no operation), cuesta 4 ciclos
      // JP imm16: salto incondicional a una direccion de 16bits
      0xC3 => {
        let address = mmu.read_u16(self.pc);
        self.pc = address;
        16
      },
      // CP a, n8: compara el registro de A con un valor inmediato restando sin guardar el resultado, actualiza el flag Z.
      0xFE => {
        let valor = mmu.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let resultado = self.a.wrapping_sub(valor);
        self.set_flag_z(resultado == 0);
        8
      },
      // JR Z, e8: salto relativo condicional, salta solo si el flag Z está activo
      0x28 => {
        let salto = mmu.read(self.pc) as i8;
        self.pc = self.pc.wrapping_add(1);
        if self.get_flag_z() {
          self.pc = self.pc.wrapping_add(salto as u16);
          12
        } else {
          8
        }
      },
      //  XOR A, A: hace XOR de A consigo mismo, siempre resulta en 0, activa el flag Z
      0xaf => {
        self.a ^= self.a;
        self.set_flag_z(true);
        4
      },
      // JR e8: salto relativo incondicional, siempre salta
      0x18 => {
        let salto = mmu.read(self.pc) as i8;
        self.pc = self.pc.wrapping_add(1);
        self.pc = self.pc.wrapping_add(salto as u16);
        12
      },
      // LDH [a8], A: escribe el valor de A en la dirección 0xFF00 + a8 (registros de hardware)
      0xe0 => {
        let siguiente = mmu.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let direccion = 0xFF00 + siguiente as u16;
        mmu.write(direccion, self.a);
        12
      },
      // LD A, n8: carga un valor inmediato directamente en el registro A
      0x3e => {
        let siguiente = mmu.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        self.a = siguiente;
        8
      },
      // DI: deshabilita las interrupciones, pendiente de implementar
      0xf3 => {
        4
      },
      // LD [a16], A: escribe el valor de A en una dirección de memoria de 16 bits
      0xea => {
        let direccion = mmu.read_u16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        mmu.write(direccion, self.a);
        16
      },
      // LDH A, [a8]: lee desde la dirección 0xFF00 + a8 y lo guarda en A
      0xf0 => {
        let siguiente = mmu.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let direccion = 0xFF00 + siguiente as u16;
        self.a = mmu.read(direccion);
        12
      },
      // JR NZ, e8: salto relativo condicional, salta solo si el flag Z NO está activo
      0x20 => {
        let salto = mmu.read(self.pc) as i8;
        self.pc = self.pc.wrapping_add(1);
        if !self.get_flag_z() {
          self.pc = self.pc.wrapping_add(salto as u16);
          12
        } else {
          8
        }
      },
      _ => {
        panic!("Opcode no implementado: {:#04x}", opcode)
      }
    }
  }
}