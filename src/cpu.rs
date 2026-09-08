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
    pub sp: u16,
    ime: bool,
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            a: 0x11,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            pc: 0x0100,
            sp: 0xFFFE,
            ime: false,
        }
    }

    fn get_flag_z(&self) -> bool {
        self.f & 0b10000000 != 0
    }
    fn set_flag_z(&mut self, v: bool) {
        if v {
            self.f |= 0b10000000;
        } else {
            self.f &= !0b10000000;
        }
    }
    fn get_flag_c(&self) -> bool {
        self.f & 0b00010000 != 0
    }
    fn set_flag_c(&mut self, v: bool) {
        if v {
            self.f |= 0b00010000;
        } else {
            self.f &= !0b00010000;
        }
    }
    fn set_flag_n(&mut self, v: bool) {
        if v {
            self.f |= 0b01000000;
        } else {
            self.f &= !0b01000000;
        }
    }
    fn set_flag_h(&mut self, v: bool) {
        if v {
            self.f |= 0b00100000;
        } else {
            self.f &= !0b00100000;
        }
    }

    fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }
    fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }
    fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }
    fn set_hl(&mut self, v: u16) {
        self.h = (v >> 8) as u8;
        self.l = (v & 0xFF) as u8;
    }
    fn set_bc(&mut self, v: u16) {
        self.b = (v >> 8) as u8;
        self.c = (v & 0xFF) as u8;
    }
    fn set_de(&mut self, v: u16) {
        self.d = (v >> 8) as u8;
        self.e = (v & 0xFF) as u8;
    }

    pub fn handle_interrupts(&mut self, mmu: &mut MMU) {
        if !self.ime {
            return;
        }
        let triggered = mmu.ie & mmu.if_;
        if triggered == 0 {
            return;
        }
        self.ime = false;
        if triggered & 0x04 != 0 {
            mmu.if_ &= !0x04;
            self.push(mmu, self.pc);
            self.pc = 0x0050; // timer interrupt handler
        }
        if triggered & 0x01 != 0 {
            mmu.if_ &= !0x01;
            self.sp = self.sp.wrapping_sub(2);
            mmu.write(self.sp.wrapping_add(1), (self.pc >> 8) as u8);
            mmu.write(self.sp, (self.pc & 0xFF) as u8);
            self.pc = 0x0040;
        }
    }

    fn push(&mut self, mmu: &mut MMU, val: u16) {
        self.sp = self.sp.wrapping_sub(2);
        mmu.write(self.sp.wrapping_add(1), (val >> 8) as u8);
        mmu.write(self.sp, (val & 0xFF) as u8);
    }

    fn pop(&mut self, mmu: &mut MMU) -> u16 {
        let lo = mmu.read(self.sp) as u16;
        let hi = mmu.read(self.sp.wrapping_add(1)) as u16;
        self.sp = self.sp.wrapping_add(2);
        (hi << 8) | lo
    }

    pub fn step(&mut self, mmu: &mut MMU) -> u8 {
        let opcode = mmu.read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        match opcode {
            0x00 => 4, // NOP
            0x01 => {
                let v = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                self.set_bc(v);
                12
            } // LD BC, n16
            0x02 => {
                mmu.write(self.bc(), self.a);
                8
            } // LD [BC], A
            0x03 => {
                self.set_bc(self.bc().wrapping_add(1));
                8
            } // INC BC
            0x04 => {
                self.b = self.b.wrapping_add(1);
                self.set_flag_z(self.b == 0);
                4
            } // INC B
            0x05 => {
                self.b = self.b.wrapping_sub(1);
                self.set_flag_z(self.b == 0);
                4
            } // DEC B
            0x06 => {
                self.b = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                8
            } // LD B, n8
            0x07 => {
                let b7 = self.a >> 7;
                self.a = (self.a << 1) | b7;
                self.set_flag_z(false);
                self.set_flag_c(b7 != 0);
                4
            } // RLCA
            0x08 => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                mmu.write(a, (self.sp & 0xFF) as u8);
                mmu.write(a.wrapping_add(1), (self.sp >> 8) as u8);
                20
            } // LD [a16], SP
            0x09 => {
                let r = self.hl().wrapping_add(self.bc());
                self.set_hl(r);
                8
            } // ADD HL, BC
            0x0a => {
                self.a = mmu.read(self.bc());
                8
            } // LD A, [BC]
            0x0b => {
                self.set_bc(self.bc().wrapping_sub(1));
                8
            } // DEC BC
            0x0c => {
                self.c = self.c.wrapping_add(1);
                self.set_flag_z(self.c == 0);
                4
            } // INC C
            0x0d => {
                self.c = self.c.wrapping_sub(1);
                self.set_flag_z(self.c == 0);
                4
            } // DEC C
            0x0e => {
                self.c = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                8
            } // LD C, n8
            0x10 => 4, // STOP
            0x11 => {
                let v = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                self.set_de(v);
                12
            } // LD DE, n16
            0x12 => {
                mmu.write(self.de(), self.a);
                8
            } // LD [DE], A
            0x13 => {
                self.set_de(self.de().wrapping_add(1));
                8
            } // INC DE
            0x14 => {
                self.d = self.d.wrapping_add(1);
                self.set_flag_z(self.d == 0);
                4
            } // INC D
            0x15 => {
                self.d = self.d.wrapping_sub(1);
                self.set_flag_z(self.d == 0);
                4
            } // DEC D
            0x16 => {
                self.d = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                8
            } // LD D, n8
            0x18 => {
                let s = mmu.read(self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                self.pc = self.pc.wrapping_add(s as u16);
                12
            } // JR e8
            0x19 => {
                let r = self.hl().wrapping_add(self.de());
                self.set_hl(r);
                8
            } // ADD HL, DE
            0x1a => {
                self.a = mmu.read(self.de());
                8
            } // LD A, [DE]
            0x1b => {
                self.set_de(self.de().wrapping_sub(1));
                8
            } // DEC DE
            0x1c => {
                self.e = self.e.wrapping_add(1);
                self.set_flag_z(self.e == 0);
                4
            } // INC E
            0x1d => {
                self.e = self.e.wrapping_sub(1);
                self.set_flag_z(self.e == 0);
                4
            } // DEC E
            0x1e => {
                self.e = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                8
            } // LD E, n8
            0x20 => {
                // JR NZ
                let s = mmu.read(self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                if !self.get_flag_z() {
                    self.pc = self.pc.wrapping_add(s as u16);
                    12
                } else {
                    8
                }
            }
            0x21 => {
                let v = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                self.set_hl(v);
                12
            } // LD HL, n16
            0x22 => {
                let a = self.hl();
                mmu.write(a, self.a);
                self.set_hl(a.wrapping_add(1));
                8
            } // LD [HL+], A
            0x23 => {
                self.set_hl(self.hl().wrapping_add(1));
                8
            } // INC HL
            0x24 => {
                self.h = self.h.wrapping_add(1);
                self.set_flag_z(self.h == 0);
                4
            } // INC H
            0x25 => {
                self.h = self.h.wrapping_sub(1);
                self.set_flag_z(self.h == 0);
                4
            } // DEC H
            0x26 => {
                self.h = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                8
            } // LD H, n8
            0x28 => {
                // JR Z
                let s = mmu.read(self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                if self.get_flag_z() {
                    self.pc = self.pc.wrapping_add(s as u16);
                    12
                } else {
                    8
                }
            }
            0x29 => {
                let r = self.hl().wrapping_add(self.hl());
                self.set_hl(r);
                8
            } // ADD HL, HL
            0x2a => {
                let a = self.hl();
                self.a = mmu.read(a);
                self.set_hl(a.wrapping_add(1));
                8
            } // LD A, [HL+]
            0x2b => {
                self.set_hl(self.hl().wrapping_sub(1));
                8
            } // DEC HL
            0x2c => {
                self.l = self.l.wrapping_add(1);
                self.set_flag_z(self.l == 0);
                4
            } // INC L
            0x2d => {
                self.l = self.l.wrapping_sub(1);
                self.set_flag_z(self.l == 0);
                4
            } // DEC L
            0x2e => {
                self.l = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                8
            } // LD L, n8
            0x2f => {
                self.a = !self.a;
                self.set_flag_n(true);
                self.set_flag_h(true);
                4
            } // CPL
            0x30 => {
                // JR NC
                let s = mmu.read(self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                if !self.get_flag_c() {
                    self.pc = self.pc.wrapping_add(s as u16);
                    12
                } else {
                    8
                }
            }
            0x31 => {
                self.sp = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                12
            } // LD SP, n16
            0x32 => {
                let a = self.hl();
                mmu.write(a, self.a);
                self.set_hl(a.wrapping_sub(1));
                8
            } // LD [HL-], A
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                8
            } // INC SP
            0x34 => {
                let a = self.hl();
                let v = mmu.read(a).wrapping_add(1);
                mmu.write(a, v);
                self.set_flag_z(v == 0);
                12
            } // INC [HL]
            0x35 => {
                let a = self.hl();
                let v = mmu.read(a).wrapping_sub(1);
                mmu.write(a, v);
                self.set_flag_z(v == 0);
                12
            } // DEC [HL]
            0x36 => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                mmu.write(self.hl(), v);
                12
            } // LD [HL], n8
            0x38 => {
                // JR C
                let s = mmu.read(self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                if self.get_flag_c() {
                    self.pc = self.pc.wrapping_add(s as u16);
                    12
                } else {
                    8
                }
            }
            0x39 => {
                let r = self.hl().wrapping_add(self.sp);
                self.set_hl(r);
                8
            } // ADD HL, SP
            0x3a => {
                let a = self.hl();
                self.a = mmu.read(a);
                self.set_hl(a.wrapping_sub(1));
                8
            } // LD A, [HL-]
            0x3b => {
                self.sp = self.sp.wrapping_sub(1);
                8
            } // DEC SP
            0x3c => {
                self.a = self.a.wrapping_add(1);
                self.set_flag_z(self.a == 0);
                4
            } // INC A
            0x3d => {
                self.a = self.a.wrapping_sub(1);
                self.set_flag_z(self.a == 0);
                4
            } // DEC A
            0x3e => {
                self.a = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                8
            } // LD A, n8
            // LD r8, r8 block
            0x40 => 4, // LD B, B
            0x41 => {
                self.b = self.c;
                4
            } // LD B, C
            0x42 => {
                self.b = self.d;
                4
            } // LD B, D
            0x43 => {
                self.b = self.e;
                4
            } // LD B, E
            0x44 => {
                self.b = self.h;
                4
            } // LD B, H
            0x45 => {
                self.b = self.l;
                4
            } // LD B, L
            0x46 => {
                self.b = mmu.read(self.hl());
                8
            } // LD B, [HL]
            0x47 => {
                self.b = self.a;
                4
            } // LD B, A
            0x48 => {
                self.c = self.b;
                4
            } // LD C, B
            0x49 => 4, // LD C, C
            0x4a => {
                self.c = self.d;
                4
            } // LD C, D
            0x4b => {
                self.c = self.e;
                4
            } // LD C, E
            0x4c => {
                self.c = self.h;
                4
            } // LD C, H
            0x4d => {
                self.c = self.l;
                4
            } // LD C, L
            0x4e => {
                self.c = mmu.read(self.hl());
                8
            } // LD C, [HL]
            0x4f => {
                self.c = self.a;
                4
            } // LD C, A
            0x50 => {
                self.d = self.b;
                4
            } // LD D, B
            0x51 => {
                self.d = self.c;
                4
            } // LD D, C
            0x52 => 4, // LD D, D
            0x53 => {
                self.d = self.e;
                4
            } // LD D, E
            0x54 => {
                self.d = self.h;
                4
            } // LD D, H
            0x55 => {
                self.d = self.l;
                4
            } // LD D, L
            0x56 => {
                self.d = mmu.read(self.hl());
                8
            } // LD D, [HL]
            0x57 => {
                self.d = self.a;
                4
            } // LD D, A
            0x58 => {
                self.e = self.b;
                4
            } // LD E, B
            0x59 => {
                self.e = self.c;
                4
            } // LD E, C
            0x5a => {
                self.e = self.d;
                4
            } // LD E, D
            0x5b => 4, // LD E, E
            0x5c => {
                self.e = self.h;
                4
            } // LD E, H
            0x5d => {
                self.e = self.l;
                4
            } // LD E, L
            0x5e => {
                self.e = mmu.read(self.hl());
                8
            } // LD E, [HL]
            0x5f => {
                self.e = self.a;
                4
            } // LD E, A
            0x60 => {
                self.h = self.b;
                4
            } // LD H, B
            0x61 => {
                self.h = self.c;
                4
            } // LD H, C
            0x62 => {
                self.h = self.d;
                4
            } // LD H, D
            0x63 => {
                self.h = self.e;
                4
            } // LD H, E
            0x64 => 4, // LD H, H
            0x65 => {
                self.h = self.l;
                4
            } // LD H, L
            0x66 => {
                self.h = mmu.read(self.hl());
                8
            } // LD H, [HL]
            0x67 => {
                self.h = self.a;
                4
            } // LD H, A
            0x68 => {
                self.l = self.b;
                4
            } // LD L, B
            0x69 => {
                self.l = self.c;
                4
            } // LD L, C
            0x6a => {
                self.l = self.d;
                4
            } // LD L, D
            0x6b => {
                self.l = self.e;
                4
            } // LD L, E
            0x6c => {
                self.l = self.h;
                4
            } // LD L, H
            0x6d => 4, // LD L, L
            0x6e => {
                self.l = mmu.read(self.hl());
                8
            } // LD L, [HL]
            0x6f => {
                self.l = self.a;
                4
            } // LD L, A
            0x70 => {
                mmu.write(self.hl(), self.b);
                8
            } // LD [HL], B
            0x71 => {
                mmu.write(self.hl(), self.c);
                8
            } // LD [HL], C
            0x72 => {
                mmu.write(self.hl(), self.d);
                8
            } // LD [HL], D
            0x73 => {
                mmu.write(self.hl(), self.e);
                8
            } // LD [HL], E
            0x74 => {
                mmu.write(self.hl(), self.h);
                8
            } // LD [HL], H
            0x75 => {
                mmu.write(self.hl(), self.l);
                8
            } // LD [HL], L
            0x76 => 4, // HALT (pendiente)
            0x77 => {
                mmu.write(self.hl(), self.a);
                8
            } // LD [HL], A
            0x78 => {
                self.a = self.b;
                4
            } // LD A, B
            0x79 => {
                self.a = self.c;
                4
            } // LD A, C
            0x7a => {
                self.a = self.d;
                4
            } // LD A, D
            0x7b => {
                self.a = self.e;
                4
            } // LD A, E
            0x7c => {
                self.a = self.h;
                4
            } // LD A, H
            0x7d => {
                self.a = self.l;
                4
            } // LD A, L
            0x7e => {
                self.a = mmu.read(self.hl());
                8
            } // LD A, [HL]
            0x7f => 4, // LD A, A
            // ADD A, r8
            0x80 => {
                let r = self.a.wrapping_add(self.b);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                4
            }
            0x81 => {
                let r = self.a.wrapping_add(self.c);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                4
            }
            0x82 => {
                let r = self.a.wrapping_add(self.d);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                4
            }
            0x83 => {
                let r = self.a.wrapping_add(self.e);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                4
            }
            0x84 => {
                let r = self.a.wrapping_add(self.h);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                4
            }
            0x85 => {
                let r = self.a.wrapping_add(self.l);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                4
            }
            0x86 => {
                let v = mmu.read(self.hl());
                let r = self.a.wrapping_add(v);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                8
            }
            0x87 => {
                let r = self.a.wrapping_add(self.a);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a > 127);
                self.a = r;
                4
            }
            // SUB A, r8
            0x90 => {
                let r = self.a.wrapping_sub(self.b);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < self.b);
                self.a = r;
                4
            }
            0x91 => {
                let r = self.a.wrapping_sub(self.c);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < self.c);
                self.a = r;
                4
            }
            0x92 => {
                let r = self.a.wrapping_sub(self.d);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < self.d);
                self.a = r;
                4
            }
            0x93 => {
                let r = self.a.wrapping_sub(self.e);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < self.e);
                self.a = r;
                4
            }
            0x94 => {
                let r = self.a.wrapping_sub(self.h);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < self.h);
                self.a = r;
                4
            }
            0x95 => {
                let r = self.a.wrapping_sub(self.l);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < self.l);
                self.a = r;
                4
            }
            0x96 => {
                let v = mmu.read(self.hl());
                let r = self.a.wrapping_sub(v);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < v);
                self.a = r;
                8
            }
            0x97 => {
                self.a = 0;
                self.set_flag_z(true);
                self.set_flag_c(false);
                4
            }
            // SBC A, r8
            0x98 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_sub(self.b).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x9c => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_sub(self.h).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            // AND A, r8
            0xa0 => {
                self.a &= self.b;
                self.set_flag_z(self.a == 0);
                4
            }
            0xa1 => {
                self.a &= self.c;
                self.set_flag_z(self.a == 0);
                4
            }
            0xa2 => {
                self.a &= self.d;
                self.set_flag_z(self.a == 0);
                4
            }
            0xa3 => {
                self.a &= self.e;
                self.set_flag_z(self.a == 0);
                4
            }
            0xa4 => {
                self.a &= self.h;
                self.set_flag_z(self.a == 0);
                4
            }
            0xa5 => {
                self.a &= self.l;
                self.set_flag_z(self.a == 0);
                4
            }
            0xa6 => {
                self.a &= mmu.read(self.hl());
                self.set_flag_z(self.a == 0);
                8
            }
            0xa7 => {
                self.set_flag_z(self.a == 0);
                4
            }
            // XOR A, r8
            0xa8 => {
                self.a ^= self.b;
                self.set_flag_z(self.a == 0);
                4
            }
            0xa9 => {
                self.a ^= self.c;
                self.set_flag_z(self.a == 0);
                4
            }
            0xaa => {
                self.a ^= self.d;
                self.set_flag_z(self.a == 0);
                4
            }
            0xab => {
                self.a ^= self.e;
                self.set_flag_z(self.a == 0);
                4
            }
            0xac => {
                self.a ^= self.h;
                self.set_flag_z(self.a == 0);
                4
            }
            0xad => {
                self.a ^= self.l;
                self.set_flag_z(self.a == 0);
                4
            }
            0xae => {
                self.a ^= mmu.read(self.hl());
                self.set_flag_z(self.a == 0);
                8
            }
            0xaf => {
                self.a = 0;
                self.set_flag_z(true);
                4
            } // XOR A, A
            // OR A, r8
            0xb0 => {
                self.a |= self.b;
                self.set_flag_z(self.a == 0);
                4
            }
            0xb1 => {
                self.a |= self.c;
                self.set_flag_z(self.a == 0);
                4
            }
            0xb2 => {
                self.a |= self.d;
                self.set_flag_z(self.a == 0);
                4
            }
            0xb3 => {
                self.a |= self.e;
                self.set_flag_z(self.a == 0);
                4
            }
            0xb4 => {
                self.a |= self.h;
                self.set_flag_z(self.a == 0);
                4
            }
            0xb5 => {
                self.a |= self.l;
                self.set_flag_z(self.a == 0);
                4
            }
            0xb6 => {
                self.a |= mmu.read(self.hl());
                self.set_flag_z(self.a == 0);
                8
            }
            0xb7 => {
                self.set_flag_z(self.a == 0);
                4
            }
            // CP A, r8
            0xb8 => {
                self.set_flag_z(self.a.wrapping_sub(self.b) == 0);
                4
            }
            0xb9 => {
                self.set_flag_z(self.a.wrapping_sub(self.c) == 0);
                4
            }
            0xba => {
                self.set_flag_z(self.a.wrapping_sub(self.d) == 0);
                4
            }
            0xbb => {
                self.set_flag_z(self.a.wrapping_sub(self.e) == 0);
                4
            }
            0xbc => {
                self.set_flag_z(self.a.wrapping_sub(self.h) == 0);
                4
            }
            0xbd => {
                self.set_flag_z(self.a.wrapping_sub(self.l) == 0);
                4
            }
            0xbe => {
                let v = mmu.read(self.hl());
                self.set_flag_z(self.a.wrapping_sub(v) == 0);
                8
            }
            0xbf => {
                self.set_flag_z(true);
                4
            } // CP A, A
            // Control flow
            0xc0 => {
                let v = self.pop(mmu);
                if !self.get_flag_z() {
                    self.pc = v;
                    return 20;
                }
                8
            } // RET NZ
            0xc1 => {
                let v = self.pop(mmu);
                self.set_bc(v);
                12
            } // POP BC
            0xc2 => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if !self.get_flag_z() {
                    self.pc = a;
                    return 16;
                }
                12
            } // JP NZ
            0xc3 => {
                self.pc = mmu.read_u16(self.pc);
                16
            } // JP nn
            0xc4 => {
                // CALL NZ
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if !self.get_flag_z() {
                    self.push(mmu, self.pc);
                    self.pc = a;
                    return 24;
                }
                12
            }
            0xc5 => {
                let v = self.bc();
                self.push(mmu, v);
                16
            } // PUSH BC
            0xc7 => {
                self.push(mmu, self.pc);
                self.pc = 0x0000;
                16
            } // RST $00
            0xc8 => {
                let v = self.pop(mmu);
                if self.get_flag_z() {
                    self.pc = v;
                    return 20;
                }
                8
            } // RET Z
            0xc9 => {
                self.pc = self.pop(mmu);
                16
            } // RET
            0xca => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if self.get_flag_z() {
                    self.pc = a;
                    return 16;
                }
                12
            } // JP Z
            0xcb => {
                // PREFIX CB
                let sub = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.step_cb(sub, mmu)
            }
            0xcc => {
                // CALL Z
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if self.get_flag_z() {
                    self.push(mmu, self.pc);
                    self.pc = a;
                    return 24;
                }
                12
            }
            0xcd => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                self.push(mmu, self.pc);
                self.pc = a;
                24
            } // CALL nn
            0xcf => {
                self.push(mmu, self.pc);
                self.pc = 0x0008;
                16
            } // RST $08
            0xd0 => {
                let v = self.pop(mmu);
                if !self.get_flag_c() {
                    self.pc = v;
                    return 20;
                }
                8
            } // RET NC
            0xd1 => {
                let v = self.pop(mmu);
                self.set_de(v);
                12
            } // POP DE
            0xd2 => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if !self.get_flag_c() {
                    self.pc = a;
                    return 16;
                }
                12
            } // JP NC
            0xd4 => {
                // CALL NC
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if !self.get_flag_c() {
                    self.push(mmu, self.pc);
                    self.pc = a;
                    return 24;
                }
                12
            }
            0xd5 => {
                let v = self.de();
                self.push(mmu, v);
                16
            } // PUSH DE
            0xd6 => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let r = self.a.wrapping_sub(v);
                self.set_flag_z(r == 0);
                self.set_flag_c(self.a < v);
                self.a = r;
                8
            } // SUB n8
            0xd7 => {
                self.push(mmu, self.pc);
                self.pc = 0x0010;
                16
            } // RST $10
            0xd8 => {
                let v = self.pop(mmu);
                if self.get_flag_c() {
                    self.pc = v;
                    return 20;
                }
                8
            } // RET C
            0xd9 => {
                self.pc = self.pop(mmu);
                self.ime = true;
                16
            } // RETI
            0xda => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if self.get_flag_c() {
                    self.pc = a;
                    return 16;
                }
                12
            } // JP C
            0xdc => {
                // CALL C
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                if self.get_flag_c() {
                    self.push(mmu, self.pc);
                    self.pc = a;
                    return 24;
                }
                12
            }
            0xdf => {
                self.push(mmu, self.pc);
                self.pc = 0x0018;
                16
            } // RST $18
            0xe0 => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                mmu.write(0xFF00 + v as u16, self.a);
                12
            } // LDH [n8], A
            0xe1 => {
                let v = self.pop(mmu);
                self.set_hl(v);
                12
            } // POP HL
            0xe2 => {
                mmu.write(0xFF00 + self.c as u16, self.a);
                8
            } // LDH [C], A
            0xe4 => {
                self.pc = self.pc.wrapping_add(1);
                8
            } // invalid, skip
            0xe5 => {
                let v = self.hl();
                self.push(mmu, v);
                16
            } // PUSH HL
            0xe6 => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a &= v;
                self.set_flag_z(self.a == 0);
                8
            } // AND n8
            0xe7 => {
                self.push(mmu, self.pc);
                self.pc = 0x0020;
                16
            } // RST $20
            0xe8 => {
                let v = mmu.read(self.pc) as i8 as i16;
                self.pc = self.pc.wrapping_add(1);
                self.sp = (self.sp as i32).wrapping_add(v as i32) as u16;
                16
            } // ADD SP, e8
            0xe9 => {
                self.pc = self.hl();
                4
            } // JP HL
            0xea => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                mmu.write(a, self.a);
                16
            } // LD [a16], A
            0xee => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a ^= v;
                self.set_flag_z(self.a == 0);
                8
            } // XOR n8
            0xef => {
                self.push(mmu, self.pc);
                self.pc = 0x0028;
                16
            } // RST $28
            0xf0 => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a = mmu.read(0xFF00 + v as u16);
                12
            } // LDH A, [n8]
            0xf1 => {
                let v = self.pop(mmu);
                self.f = (v & 0xFF) as u8;
                self.a = (v >> 8) as u8;
                12
            } // POP AF
            0xf2 => {
                self.a = mmu.read(0xFF00 + self.c as u16);
                8
            } // LDH A, [C]
            0xf3 => {
                self.ime = false;
                4
            } // DI
            0xf5 => {
                let v = ((self.a as u16) << 8) | (self.f as u16);
                self.push(mmu, v);
                16
            } // PUSH AF
            0xf7 => {
                self.push(mmu, self.pc);
                self.pc = 0x0030;
                16
            } // RST $30
            0xf8 => {
                let v = mmu.read(self.pc) as i8 as i16;
                self.pc = self.pc.wrapping_add(1);
                let r = (self.sp as i32).wrapping_add(v as i32) as u16;
                self.set_hl(r);
                12
            } // LD HL, SP+e8
            0xf9 => {
                self.sp = self.hl();
                8
            } // LD SP, HL
            0xfa => {
                let a = mmu.read_u16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                self.a = mmu.read(a);
                16
            } // LD A, [a16]
            0xfb => {
                self.ime = true;
                4
            } // EI
            0xfe => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let r = self.a.wrapping_sub(v);
                self.set_flag_z(r == 0);
                8
            } // CP n8
            0xff => {
                self.push(mmu, self.pc);
                self.pc = 0x0038;
                16
            } // RST $38
            // ADC A, r8
            0x88 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_add(self.b).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x89 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_add(self.c).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x8a => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_add(self.d).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x8b => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_add(self.e).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x8c => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_add(self.h).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x8d => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_add(self.l).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x8e => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let v = mmu.read(self.hl());
                let r = self.a.wrapping_add(v).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                8
            }
            0x8f => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_add(self.a).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            // SBC A, r8 faltantes
            0x99 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_sub(self.c).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x9a => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_sub(self.d).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x9b => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_sub(self.e).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x9d => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_sub(self.l).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            0x9e => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let v = mmu.read(self.hl());
                let r = self.a.wrapping_sub(v).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                8
            }
            0x9f => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let r = self.a.wrapping_sub(self.a).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                4
            }
            // Instrucciones sueltas
            0x0f => {
                let b0 = self.a & 1;
                self.a = (self.a >> 1) | (b0 << 7);
                self.set_flag_z(false);
                self.set_flag_c(b0 != 0);
                4
            } // RRCA
            0x17 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b7 = self.a >> 7;
                self.a = (self.a << 1) | c;
                self.set_flag_z(false);
                self.set_flag_c(b7 != 0);
                4
            } // RLA
            0x1f => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b0 = self.a & 1;
                self.a = (self.a >> 1) | c;
                self.set_flag_z(false);
                self.set_flag_c(b0 != 0);
                4
            } // RRA
            0x37 => {
                self.set_flag_c(true);
                self.set_flag_n(false);
                self.set_flag_h(false);
                4
            } // SCF
            0x3f => {
                let c = self.get_flag_c();
                self.set_flag_c(!c);
                self.set_flag_n(false);
                self.set_flag_h(false);
                4
            } // CCF
            0x27 => 4, // DAA pendiente
            0xc6 => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let r = self.a.wrapping_add(v);
                self.set_flag_z(r == 0);
                self.set_flag_c(r < self.a);
                self.a = r;
                8
            } // ADD A, n8
            0xce => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let r = self.a.wrapping_add(v).wrapping_add(c);
                self.set_flag_z(r == 0);
                self.a = r;
                8
            } // ADC A, n8
            0xde => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let r = self.a.wrapping_sub(v).wrapping_sub(c);
                self.set_flag_z(r == 0);
                self.a = r;
                8
            } // SBC A, n8
            0xf6 => {
                let v = mmu.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a |= v;
                self.set_flag_z(self.a == 0);
                8
            } // OR A, n8
            _ => {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open("bruma.log")
                    .unwrap();
                writeln!(
                    file,
                    "Opcode no implementado: {:#04x} en PC: {:#06x}",
                    opcode,
                    self.pc.wrapping_sub(1)
                )
                .unwrap();
                4
            }
        }
    }
    fn step_cb(&mut self, sub: u8, mmu: &mut MMU) -> u8 {
        // Registro por índice: 0=B, 1=C, 2=D, 3=E, 4=H, 5=L, 6=[HL], 7=A
        let bit = (sub >> 3) & 0x07; // bit index para BIT/RES/SET
        let reg = sub & 0x07; // registro

        match sub {
            // RLC r8 (0x00-0x07)
            0x00 => {
                let b = self.b >> 7;
                self.b = (self.b << 1) | b;
                self.set_flag_z(self.b == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x01 => {
                let b = self.c >> 7;
                self.c = (self.c << 1) | b;
                self.set_flag_z(self.c == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x02 => {
                let b = self.d >> 7;
                self.d = (self.d << 1) | b;
                self.set_flag_z(self.d == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x03 => {
                let b = self.e >> 7;
                self.e = (self.e << 1) | b;
                self.set_flag_z(self.e == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x04 => {
                let b = self.h >> 7;
                self.h = (self.h << 1) | b;
                self.set_flag_z(self.h == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x05 => {
                let b = self.l >> 7;
                self.l = (self.l << 1) | b;
                self.set_flag_z(self.l == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x06 => {
                let a = self.hl();
                let v = mmu.read(a);
                let b = v >> 7;
                let r = (v << 1) | b;
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                self.set_flag_c(b != 0);
                16
            }
            0x07 => {
                let b = self.a >> 7;
                self.a = (self.a << 1) | b;
                self.set_flag_z(self.a == 0);
                self.set_flag_c(b != 0);
                8
            }
            // RRC r8 (0x08-0x0F)
            0x08 => {
                let b = self.b & 1;
                self.b = (self.b >> 1) | (b << 7);
                self.set_flag_z(self.b == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x09 => {
                let b = self.c & 1;
                self.c = (self.c >> 1) | (b << 7);
                self.set_flag_z(self.c == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x0a => {
                let b = self.d & 1;
                self.d = (self.d >> 1) | (b << 7);
                self.set_flag_z(self.d == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x0b => {
                let b = self.e & 1;
                self.e = (self.e >> 1) | (b << 7);
                self.set_flag_z(self.e == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x0c => {
                let b = self.h & 1;
                self.h = (self.h >> 1) | (b << 7);
                self.set_flag_z(self.h == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x0d => {
                let b = self.l & 1;
                self.l = (self.l >> 1) | (b << 7);
                self.set_flag_z(self.l == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x0e => {
                let a = self.hl();
                let v = mmu.read(a);
                let b = v & 1;
                let r = (v >> 1) | (b << 7);
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                self.set_flag_c(b != 0);
                16
            }
            0x0f => {
                let b = self.a & 1;
                self.a = (self.a >> 1) | (b << 7);
                self.set_flag_z(self.a == 0);
                self.set_flag_c(b != 0);
                8
            }
            // RL r8 (0x10-0x17)
            0x10 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = self.b >> 7;
                self.b = (self.b << 1) | c;
                self.set_flag_z(self.b == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x11 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = self.c >> 7;
                self.c = (self.c << 1) | c;
                self.set_flag_z(self.c == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x12 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = self.d >> 7;
                self.d = (self.d << 1) | c;
                self.set_flag_z(self.d == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x13 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = self.e >> 7;
                self.e = (self.e << 1) | c;
                self.set_flag_z(self.e == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x14 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = self.h >> 7;
                self.h = (self.h << 1) | c;
                self.set_flag_z(self.h == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x15 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = self.l >> 7;
                self.l = (self.l << 1) | c;
                self.set_flag_z(self.l == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x16 => {
                let a = self.hl();
                let v = mmu.read(a);
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = v >> 7;
                let r = (v << 1) | c;
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                self.set_flag_c(b != 0);
                16
            }
            0x17 => {
                let c = if self.get_flag_c() { 1u8 } else { 0 };
                let b = self.a >> 7;
                self.a = (self.a << 1) | c;
                self.set_flag_z(self.a == 0);
                self.set_flag_c(b != 0);
                8
            }
            // RR r8 (0x18-0x1F)
            0x18 => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = self.b & 1;
                self.b = (self.b >> 1) | c;
                self.set_flag_z(self.b == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x19 => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = self.c & 1;
                self.c = (self.c >> 1) | c;
                self.set_flag_z(self.c == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x1a => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = self.d & 1;
                self.d = (self.d >> 1) | c;
                self.set_flag_z(self.d == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x1b => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = self.e & 1;
                self.e = (self.e >> 1) | c;
                self.set_flag_z(self.e == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x1c => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = self.h & 1;
                self.h = (self.h >> 1) | c;
                self.set_flag_z(self.h == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x1d => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = self.l & 1;
                self.l = (self.l >> 1) | c;
                self.set_flag_z(self.l == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x1e => {
                let a = self.hl();
                let v = mmu.read(a);
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = v & 1;
                let r = (v >> 1) | c;
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                self.set_flag_c(b != 0);
                16
            }
            0x1f => {
                let c = if self.get_flag_c() { 0x80u8 } else { 0 };
                let b = self.a & 1;
                self.a = (self.a >> 1) | c;
                self.set_flag_z(self.a == 0);
                self.set_flag_c(b != 0);
                8
            }
            // SLA r8 (0x20-0x27)
            0x20 => {
                let b = self.b >> 7;
                self.b <<= 1;
                self.set_flag_z(self.b == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x21 => {
                let b = self.c >> 7;
                self.c <<= 1;
                self.set_flag_z(self.c == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x22 => {
                let b = self.d >> 7;
                self.d <<= 1;
                self.set_flag_z(self.d == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x23 => {
                let b = self.e >> 7;
                self.e <<= 1;
                self.set_flag_z(self.e == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x24 => {
                let b = self.h >> 7;
                self.h <<= 1;
                self.set_flag_z(self.h == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x25 => {
                let b = self.l >> 7;
                self.l <<= 1;
                self.set_flag_z(self.l == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x26 => {
                let a = self.hl();
                let v = mmu.read(a);
                let b = v >> 7;
                let r = v << 1;
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                self.set_flag_c(b != 0);
                16
            }
            0x27 => {
                let b = self.a >> 7;
                self.a <<= 1;
                self.set_flag_z(self.a == 0);
                self.set_flag_c(b != 0);
                8
            }
            // SRA r8 (0x28-0x2F)
            0x28 => {
                let b = self.b & 1;
                self.b = (self.b >> 1) | (self.b & 0x80);
                self.set_flag_z(self.b == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x29 => {
                let b = self.c & 1;
                self.c = (self.c >> 1) | (self.c & 0x80);
                self.set_flag_z(self.c == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x2a => {
                let b = self.d & 1;
                self.d = (self.d >> 1) | (self.d & 0x80);
                self.set_flag_z(self.d == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x2b => {
                let b = self.e & 1;
                self.e = (self.e >> 1) | (self.e & 0x80);
                self.set_flag_z(self.e == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x2c => {
                let b = self.h & 1;
                self.h = (self.h >> 1) | (self.h & 0x80);
                self.set_flag_z(self.h == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x2d => {
                let b = self.l & 1;
                self.l = (self.l >> 1) | (self.l & 0x80);
                self.set_flag_z(self.l == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x2e => {
                let a = self.hl();
                let v = mmu.read(a);
                let b = v & 1;
                let r = (v >> 1) | (v & 0x80);
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                self.set_flag_c(b != 0);
                16
            }
            0x2f => {
                let b = self.a & 1;
                self.a = (self.a >> 1) | (self.a & 0x80);
                self.set_flag_z(self.a == 0);
                self.set_flag_c(b != 0);
                8
            }
            // SWAP r8 (0x30-0x37)
            0x30 => {
                self.b = (self.b >> 4) | (self.b << 4);
                self.set_flag_z(self.b == 0);
                8
            }
            0x31 => {
                self.c = (self.c >> 4) | (self.c << 4);
                self.set_flag_z(self.c == 0);
                8
            }
            0x32 => {
                self.d = (self.d >> 4) | (self.d << 4);
                self.set_flag_z(self.d == 0);
                8
            }
            0x33 => {
                self.e = (self.e >> 4) | (self.e << 4);
                self.set_flag_z(self.e == 0);
                8
            }
            0x34 => {
                self.h = (self.h >> 4) | (self.h << 4);
                self.set_flag_z(self.h == 0);
                8
            }
            0x35 => {
                self.l = (self.l >> 4) | (self.l << 4);
                self.set_flag_z(self.l == 0);
                8
            }
            0x36 => {
                let a = self.hl();
                let v = mmu.read(a);
                let r = (v >> 4) | (v << 4);
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                16
            }
            0x37 => {
                self.a = (self.a >> 4) | (self.a << 4);
                self.set_flag_z(self.a == 0);
                8
            }
            // SRL r8 (0x38-0x3F)
            0x38 => {
                let b = self.b & 1;
                self.b >>= 1;
                self.set_flag_z(self.b == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x39 => {
                let b = self.c & 1;
                self.c >>= 1;
                self.set_flag_z(self.c == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x3a => {
                let b = self.d & 1;
                self.d >>= 1;
                self.set_flag_z(self.d == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x3b => {
                let b = self.e & 1;
                self.e >>= 1;
                self.set_flag_z(self.e == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x3c => {
                let b = self.h & 1;
                self.h >>= 1;
                self.set_flag_z(self.h == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x3d => {
                let b = self.l & 1;
                self.l >>= 1;
                self.set_flag_z(self.l == 0);
                self.set_flag_c(b != 0);
                8
            }
            0x3e => {
                let a = self.hl();
                let v = mmu.read(a);
                let b = v & 1;
                let r = v >> 1;
                mmu.write(a, r);
                self.set_flag_z(r == 0);
                self.set_flag_c(b != 0);
                16
            }
            0x3f => {
                let b = self.a & 1;
                self.a >>= 1;
                self.set_flag_z(self.a == 0);
                self.set_flag_c(b != 0);
                8
            }
            // BIT b, r8 (0x40-0x7F)
            0x40..=0x7f => {
                let mask = 1u8 << bit;
                let val = match reg {
                    0 => self.b,
                    1 => self.c,
                    2 => self.d,
                    3 => self.e,
                    4 => self.h,
                    5 => self.l,
                    6 => mmu.read(self.hl()),
                    _ => self.a,
                };
                self.set_flag_z(val & mask == 0);
                self.set_flag_n(false);
                self.set_flag_h(true);
                if reg == 6 { 12 } else { 8 }
            }
            // RES b, r8 (0x80-0xBF)
            0x80..=0xbf => {
                let mask = !(1u8 << bit);
                match reg {
                    0 => self.b &= mask,
                    1 => self.c &= mask,
                    2 => self.d &= mask,
                    3 => self.e &= mask,
                    4 => self.h &= mask,
                    5 => self.l &= mask,
                    6 => {
                        let a = self.hl();
                        let v = mmu.read(a) & mask;
                        mmu.write(a, v);
                    }
                    _ => self.a &= mask,
                }
                if reg == 6 { 16 } else { 8 }
            }
            // SET b, r8 (0xC0-0xFF)
            0xc0..=0xff => {
                let mask = 1u8 << bit;
                match reg {
                    0 => self.b |= mask,
                    1 => self.c |= mask,
                    2 => self.d |= mask,
                    3 => self.e |= mask,
                    4 => self.h |= mask,
                    5 => self.l |= mask,
                    6 => {
                        let a = self.hl();
                        let v = mmu.read(a) | mask;
                        mmu.write(a, v);
                    }
                    _ => self.a |= mask,
                }
                if reg == 6 { 16 } else { 8 }
            }
        }
    }
}
