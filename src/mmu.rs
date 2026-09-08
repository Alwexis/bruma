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
    ram: Vec<u8>,         // vector de bytes para la RAM
    vram: Vec<u8>,
    hram: Vec<u8>,
    linea_lcd: u8,
    pub lcdc: u8,
    pub paleta_bg: u8,
    pub ie: u8,         // Interrupt Enable - 0xFFFF
    pub if_: u8,        // Interrupt Flag - 0xFF0F
    joypad_select: u8,  // lo que el juego escribe para seleccionar
    botones_accion: u8, // A, B, Select, Start — 0 = presionado
    botones_dir: u8,    // Derecha, Izquierda, Arriba, Abajo — 0 = presionado
    div: u8,
    tima: u8,
    tma: u8,
    tac: u8,
    ciclos_div: u32,
    ciclos_timer: u32,
}

impl MMU {
    // constructor del MMU, inicializa con el cartucho especificado y con una ram de 64kb
    pub fn new(cartridge: Cartridge) -> MMU {
        MMU {
            cartridge,
            ram: vec![0u8; 0x10000],
            vram: vec![0u8; 0x2000],
            hram: vec![0u8; 0x7F],
            linea_lcd: 0,
            lcdc: 0x91,
            paleta_bg: 0xE4,
            ie: 0,
            if_: 0,
            joypad_select: 0xFF,
            botones_accion: 0xFF,
            botones_dir: 0xFF, // ninguna dirección
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            ciclos_div: 0,
            ciclos_timer: 0,
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
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize],
            0xFF44 => self.linea_lcd,
            0xFF40 => self.lcdc,
            0xFF47 => self.paleta_bg,
            0xFF0F => self.if_,
            0xFFFF => self.ie,
            0xFF00 => {
                if self.joypad_select & 0x20 == 0 {
                    self.botones_dir
                } else if self.joypad_select & 0x10 == 0 {
                    self.botones_accion
                } else {
                    0xFF
                }
            }
            0xFF04 => self.div,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac,
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
            0x8000..=0x9FFF => {
                self.vram[(address - 0x8000) as usize] = value;
            }
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize] = value,
            0x2000..=0x3FFF => self.cartridge.write_mbc(address, value),
            0xFF40 => {
                self.lcdc = value;
            }
            0xFF47 => self.paleta_bg = value,
            0xFF0F => self.if_ = value,
            0xFFFF => self.ie = value,
            0xFF00 => self.joypad_select = value,
            0xFF04 => self.div = 0,
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value,
            _ => {}
        }
    }

    pub fn tick_linea(&mut self) {
        self.linea_lcd = self.linea_lcd.wrapping_add(1);
        if self.linea_lcd > 153 {
            self.linea_lcd = 0;
        }
    }

    pub fn tick_timer(&mut self, ciclos: u8) {
        self.ciclos_div += ciclos as u32;
        if self.ciclos_div >= 256 {
            self.ciclos_div -= 256;
            self.div = self.div.wrapping_add(1);
        }

        if self.tac & 0x04 == 0 {
            return;
        } // timer desactivado

        let frecuencia = match self.tac & 0x03 {
            0 => 1024,
            1 => 16,
            2 => 64,
            _ => 256,
        };

        self.ciclos_timer += ciclos as u32;
        if self.ciclos_timer >= frecuencia {
            self.ciclos_timer -= frecuencia;
            let (nuevo, overflow) = self.tima.overflowing_add(1);
            if overflow {
                self.tima = self.tma;
                self.if_ |= 0x04; // activa interrupción de timer
            } else {
                self.tima = nuevo;
            }
        }
    }

    pub fn title(&self) -> String {
        self.cartridge.title()
    }
}
