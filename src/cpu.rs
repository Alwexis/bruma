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

  fn get_flag_c(&self) -> bool {
    self.f & 0b00010000 != 0
  }

  fn set_flag_c(&mut self, value: bool) {
    if value {
      self.f |= 0b00010000;
    } else {
      self.f &= !0b00010000;
    }
  }

  fn set_flag_n(&mut self, value: bool) {
    if value {
      self.f |= 0b01000000;
    } else {
      self.f &= !0b01000000;
    }
  }

  fn set_flag_h(&mut self, value: bool) {
    if value {
      self.f |= 0b00100000;
    } else {
      self.f &= !0b00100000;
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
      // LD HL, n16: carga un valor de 16 bits en los registros H y L. El byte alto va en H y el bajo en L
      0x21 => {
        let bytes = mmu.read_u16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.h = (bytes >> 8) as u8;
        self.l = (bytes & 0xFF) as u8;
        12
      },
      // LD SP, n16: carga un valor de 16 bits directamente en el stack pointer (sp)
      0x31 => {
        let bytes = mmu.read_u16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.sp = bytes;
        12
      },
      // LD BC, n16: carga un valor de 16 bits en los registros B y C. EL byte alto va en B y el bajo en C
      0x01 => {
        let bytes = mmu.read_u16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.b = (bytes >> 8) as u8;
        self.c = (bytes & 0xFF) as u8;
        12
      },
      // LD [HL], n8: escribe un valor inmediato en la direccion de memoria que apunta HL
      0x36 => {
        let siguiente = mmu.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let address = (self.h as u16) << 8 | self.l as u16;
        mmu.write(address, siguiente);
        12
      },
      // INC HL: incrementa el registro HL en 1 como un valor de 16bits
      0x23 => {
        let mut value = (self.h as u16) << 8 | self.l as u16;
        value = value.wrapping_add(1);
        self.h = (value >> 8) as u8;
        self.l = (value & 0xFF) as u8;
        8
      },
      // DEC BC: decrementa el registro BC en 1 como un valor de 16 bits
      0x0b => {
        let mut value = (self.b as u16) << 8 | self.c as u16;
        value = value.wrapping_sub(1);
        self.b = (value >> 8) as u8;
        self.c = (value & 0xFF) as u8;
        8
      },
      // LD A, B: copia el valor del registro B al registro A
      0x78 => {
        self.a = self.b;
        4
      },
      // OR A,C: hace OR entre A y C, guarda en A y actualiza el flag Z
      0xb1 => {
        let value = self.a | self.c;
        self.a = value;
        self.set_flag_z(value == 0);
        4
      },
      // STOP: detiene el procesador hasta una interrupción, pendiente de implementar
      0x10 => {
        4
      },
      // RET NZ: retorna de una subrutina solo si el flag Z no está activo
      0xc0 => {
        let value = mmu.read_u16(self.sp);
        self.sp = self.sp.wrapping_add(2);
        if !self.get_flag_z() {
          self.pc = value;
          return 20
        }
        8
      },
      // CALL a16: guardda el PC actual en el stack y salta a la direccion indicada
      0xcd => {
        let destino = mmu.read_u16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.sp = self.sp.wrapping_sub(2);
        mmu.write(self.sp + 1, (self.pc >> 8) as u8);  // byte alto
        mmu.write(self.sp, (self.pc & 0xFF) as u8); // byte bajo
        self.pc = destino;
        24
      },
      // PUSH AF: empuja los registros A y F al stack
      0xf5 => {
        self.sp = self.sp.wrapping_sub(2);
        mmu.write(self.sp + 1, self.a);
        mmu.write(self.sp, self.f);
        16
      },
      // POP AF: saca dos bytes del stack y los carga en F y A
      0xf1 => {
        self.f = mmu.read(self.sp);
        self.a = mmu.read(self.sp + 1);
        self.sp = self.sp.wrapping_add(2);
        12
      },
      // DEC HL: decrementa el registro HL en 1 como un valor de 16 bits
      0x2b => {
        let mut value = (self.h as u16) << 8 | self.l as u16;
        value = value.wrapping_sub(1);
        self.h = (value >> 8) as u8;
        self.l = (value & 0xFF) as u8;
        8
      },
      // JR NC, e8: salto relativo condicional, solo salta si el flag C no está activo
      0x30 => {
        let salto = mmu.read(self.pc) as i8;
        self.pc = self.pc.wrapping_add(1);
        if !self.get_flag_c() {
          self.pc = self.pc.wrapping_add(salto as u16);
          12
        } else {
          8
        }
      },
      // LD E,D: copia el valor del registro D al registro E
      0x5a => {
        self.e = self.d;
        4
      },
      // LD [BC], A: escribe el valor de A en la dirección de memoria que apunta BC
      0x02 => {
        let address = (self.b as u16) << 8 | self.c as u16;
        mmu.write(address, self.a);
        8
      },
      // LD B, L: copia el valor del registro L al registro B
      0x45 => {
        self.b = self.l;
        4
      },
      // CPL invierte todos los bits del registro A.
      0x2f => {
        self.a = !self.a;
        self.set_flag_n(true);
        self.set_flag_h(true);
        4
      },
      // LD [HL], B: escribe el valor de B en la direccion de emoria que apunta HL
      0x70 => {
        let address = (self.h as u16) << 8 | self.l as u16;
        mmu.write(address, self.b);
        8
      },
      // LD L, A: copia el valor del registro A al registro L
      0x6f => {
        self.l = self.a;
        4
      },
      // INC B: incrementa el registro B en 1, actualiza flag Z
      0x04 => {
        self.b = self.b.wrapping_add(1);
        self.set_flag_z(self.b == 0);
        4
      },
      // INC C: incrementa el registro C en 1, actualiza flag Z
      0x0c => {
        self.c = self.c.wrapping_add(1);
        self.set_flag_z(self.c == 0);
        4
      },
      // DEC B: decrementa el registro B en 1, actualiza flag Z
      0x05 => {
        self.b = self.b.wrapping_sub(1);
        self.set_flag_z(self.b == 0);
        4
      },
      // DEC C: decrementa el registro C en 1, actualiza flag Z
      0x0d => {
        self.c = self.c.wrapping_sub(1);
        self.set_flag_z(self.c == 0);
        4
      },
      // LD A, [HL]: lee el byte en la direccion que apunta HL y lo guarda en A
      0x7e => {
        let address = (self.h as u16) << 8 | self.l as u16;
        self.a = mmu.read(address);
        8
      },
      // LD H, [HL]: lee el byte en la direccion que apunta HL y lo guarda en H
      0x2a => {
        let mut address = (self.h as u16) << 8 | self.l as u16;
        self.a = mmu.read(address);
        address = address.wrapping_add(1);
        self.h = (address >> 8) as u8;
        self.l = (address & 0xFF) as u8;
        8
      },
      // LD H, [HL]: lee el byte en la direccion que apunta HL y lo guarda en H
      0x66 => {
        let address = (self.h as u16) << 8 | self.l as u16;
        self.h = mmu.read(address);
        8
      },
      // RET: retorna de una subrutina incondicional,mente
      0xc9 => {
        let value = mmu.read_u16(self.sp);
        self.sp = self.sp.wrapping_add(2);
        self.pc = value;
        return 16
      },
      // RST $10: guarda el PC en el stack y salta a la dirección fija 0x0010
      0xd7 => {
        self.sp = self.sp.wrapping_sub(2);
        mmu.write(self.sp + 1, (self.pc >> 8) as u8);  // byte alto
        mmu.write(self.sp, (self.pc & 0xFF) as u8); // byte bajo
        self.pc = 0x0010;
        16
      },
      _ => {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("bruma.log")
            .unwrap();
        writeln!(file, "Opcode no implementado: {:#04x} en PC: {:#06x}", opcode, self.pc.wrapping_sub(1)).unwrap();
        4
      }
    }
  }
}