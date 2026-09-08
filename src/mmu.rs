use crate::apu::APU;
use crate::cartridge::Cartridge;

/*
El MMU (Memory Management Unit) es el intermediario entre la CPU y todo lo que existe en memoria.
La CPU nunca le pregunta directamente al cartucho "dame tal byte", ni a la RAM "dame este otro byte";
siempre pasa por el MMU y el MMU sabe a quién preguntarle según la dirección.

Es importante el MMU porque en la GBC el mismo espacio de 64KB contiene cosas completamente distintas:
ROM del cartucho, RAM interna, VRAM, OAM (sprites), registros de hardware de video, sonido, etc.
El MMU orquesta todo este desparrame de información en un mapa de memoria unificado.

Mapa de memoria de la GBC:
  0x0000-0x3FFF  ROM banco 0 (fijo)
  0x4000-0x7FFF  ROM banco N (switcheable via mapper)
  0x8000-0x9FFF  VRAM (Video RAM — tiles y tile maps)
  0xA000-0xBFFF  RAM externa del cartucho (save data)
  0xC000-0xDFFF  RAM interna (WRAM)
  0xE000-0xFDFF  Echo RAM (espejo de 0xC000-0xDDFF)
  0xFE00-0xFE9F  OAM (Object Attribute Memory — sprites)
  0xFEA0-0xFEFF  No usable
  0xFF00-0xFF7F  Registros de hardware (I/O)
  0xFF80-0xFFFE  HRAM (High RAM — stack rápido)
  0xFFFF         Interrupt Enable register
*/
pub struct MMU {
    cartridge: Cartridge, // cartucho con ROM y RAM externa
    ram: Vec<u8>,         // RAM interna (WRAM) — 8KB visible, guardamos más por seguridad
    vram: Vec<u8>,        // Video RAM — 8KB (0x8000-0x9FFF)
    oam: Vec<u8>,         // Object Attribute Memory — 160 bytes para 40 sprites
    hram: Vec<u8>,        // High RAM — 127 bytes (0xFF80-0xFFFE)
    pub linea_lcd: u8,    // LY: línea actual del LCD (0-153), registro 0xFF44
    pub lcdc: u8,         // LCD Control — controla qué se dibuja y cómo, registro 0xFF40
    pub paleta_bg: u8,    // Paleta de fondo (4 colores), registro 0xFF47
    pub paleta_obj0: u8,  // Paleta de sprites 0, registro 0xFF48
    pub paleta_obj1: u8,  // Paleta de sprites 1, registro 0xFF49
    pub scy: u8,          // Scroll Y del fondo, registro 0xFF42
    pub scx: u8,          // Scroll X del fondo, registro 0xFF43
    pub wy: u8,           // Window Y position, registro 0xFF4A
    pub wx: u8,           // Window X position (- 7), registro 0xFF4B
    pub lyc: u8,          // LY Compare — dispara interrupción STAT cuando LY==LYC, 0xFF45
    pub ie: u8,           // Interrupt Enable — qué interrupciones están habilitadas, 0xFFFF
    pub if_: u8,          // Interrupt Flag — qué interrupciones están pendientes, 0xFF0F
    joypad_select: u8,    // Selector del joypad (escrito por el juego para elegir qué leer)
    botones_accion: u8,   // Estado de A, B, Select, Start (0 = presionado)
    botones_dir: u8,      // Estado de direcciones (0 = presionado)
    div: u8,              // Divider register — se incrementa a 16384 Hz, registro 0xFF04
    tima: u8,             // Timer counter — se incrementa según TAC, registro 0xFF05
    tma: u8,              // Timer Modulo — valor de reset cuando TIMA desborda, 0xFF06
    tac: u8,              // Timer Control — velocidad y enable del timer, 0xFF07
    ciclos_div: u32,      // contador interno para actualizar DIV
    ciclos_timer: u32,    // contador interno para actualizar TIMA
    pub stat: u8,         // LCD Status — modo PPU y flags de interrupción STAT, 0xFF41
    pub apu: APU          // la APU (revisar archivo apu.rs)
}

impl MMU {
    // Inicializa la MMU con los valores post-boot de la Game Boy
    pub fn new(cartridge: Cartridge, apu: APU) -> MMU {
        MMU {
            cartridge,
            ram: vec![0u8; 0x10000],
            vram: vec![0u8; 0x2000],
            oam: vec![0u8; 0xA0],
            hram: vec![0u8; 0x7F],
            linea_lcd: 0,
            lcdc: 0x91,      // LCD encendido, BG habilitado, tiles desde 0x8000
            paleta_bg: 0xE4, // paleta por defecto: 11 10 01 00 (negro→blanco)
            paleta_obj0: 0xE4,
            paleta_obj1: 0xE4,
            scy: 0,
            scx: 0,
            wy: 0,
            wx: 0,
            lyc: 0,
            ie: 0,
            if_: 0,
            joypad_select: 0xFF,
            botones_accion: 0xFF, // ningún botón presionado (0 = presionado, 1 = suelto)
            botones_dir: 0xFF,
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            ciclos_div: 0,
            ciclos_timer: 0,
            stat: 0x85, // modo 1 (VBlank) al inicio
            apu
        }
    }

    // Lee un byte de la dirección indicada.
    // Despacha la lectura al subsistema correcto según el rango de la dirección.
    pub fn read(&self, address: u16) -> u8 {
        match address {
            // ROM del cartucho (banco 0 y banco switcheable)
            0x0000..=0x7FFF => self.cartridge.read(address),
            // RAM externa del cartucho (save data)
            0xA000..=0xBFFF => self.cartridge.read_ram(address),
            // RAM interna
            0xC000..=0xDFFF => self.ram[(address - 0xC000) as usize],
            // Echo RAM — espejo de 0xC000-0xDDFF
            0xE000..=0xFDFF => self.ram[(address - 0xE000) as usize],
            // VRAM — tiles de gráficos y tile maps
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize],
            // OAM — datos de los 40 sprites (4 bytes cada uno)
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize],
            // Zona prohibida — siempre devuelve 0xFF
            0xFEA0..=0xFEFF => 0xFF,
            // HRAM — RAM rápida usada principalmente para el stack
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize],
            // Registros de hardware
            0xFF44 => self.linea_lcd,   // LY — línea actual
            0xFF45 => self.lyc,         // LYC — compare
            0xFF40 => self.lcdc,        // LCD Control
            0xFF42 => self.scy,         // Scroll Y
            0xFF43 => self.scx,         // Scroll X
            0xFF47 => self.paleta_bg,   // Paleta BG
            0xFF48 => self.paleta_obj0, // Paleta OBJ 0
            0xFF49 => self.paleta_obj1, // Paleta OBJ 1
            0xFF4A => self.wy,          // Window Y
            0xFF4B => self.wx,          // Window X
            0xFF0F => self.if_,         // Interrupt Flag
            0xFFFF => self.ie,          // Interrupt Enable
            // Joypad — el juego escribe a 0xFF00 para seleccionar qué botones leer (bits 4 y 5),
            // luego lee 0xFF00 para obtener el estado (0 = presionado).
            // Bit 4 (P14) = 0 -> Direcciones (Derecha=0, Izquierda=1, Arriba=2, Abajo=3)
            // Bit 5 (P15) = 0 -> Botones de acción (A=0, B=1, Select=2, Start=3)
            0xFF00 => {
                let mut val = 0xC0 | (self.joypad_select & 0x30) | 0x0F;
                if self.joypad_select & 0x10 == 0 {
                    val &= (self.botones_dir & 0x0F) | 0xF0;
                }
                if self.joypad_select & 0x20 == 0 {
                    val &= (self.botones_accion & 0x0F) | 0xF0;
                }
                val
            }
            0xFF04 => self.div,  // Divider register
            0xFF05 => self.tima, // Timer counter
            0xFF06 => self.tma,  // Timer modulo
            0xFF07 => self.tac,  // Timer control
            0xFF41 => self.stat, // LCD Status
            // Registros de Audio (APU) y Wave RAM (0xFF10 - 0xFF3F)
            0xFF10..=0xFF3F => self.apu.read(address),
            _ => 0xFF,           // Registros no implementados devuelven 0xFF
        }
    }

    // Lee dos bytes consecutivos en little-endian (byte bajo primero)
    pub fn read_u16(&self, address: u16) -> u16 {
        let low = self.read(address);
        let high = self.read(address + 1);
        ((high as u16) << 8) | (low as u16)
    }

    // Escribe un byte en la dirección indicada.
    // Algunas escrituras tienen efectos secundarios (DMA, reset de registros, etc.)
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            // Escrituras a ROM activan el mapper del cartucho
            0x0000..=0x7FFF => self.cartridge.write_mbc(address, value),
            // RAM externa del cartucho
            0xA000..=0xBFFF => self.cartridge.write_ram(address, value),
            // RAM interna
            0xC000..=0xDFFF => self.ram[(address - 0xC000) as usize] = value,
            // Echo RAM
            0xE000..=0xFDFF => self.ram[(address - 0xE000) as usize] = value,
            // VRAM
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize] = value,
            // OAM
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize] = value,
            // Zona prohibida — ignorar escrituras
            0xFEA0..=0xFEFF => {}
            // HRAM
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize] = value,
            // Registros de hardware
            0xFF40 => self.lcdc = value,
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF47 => self.paleta_bg = value,
            0xFF48 => self.paleta_obj0 = value,
            0xFF49 => self.paleta_obj1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            0xFF0F => self.if_ = value,
            0xFFFF => self.ie = value,
            0xFF00 => self.joypad_select = value,
            // Registros de Audio (APU) y Wave RAM (0xFF10 - 0xFF3F)
            0xFF10..=0xFF3F => self.apu.write(address, value),
            // Escribir cualquier valor a DIV lo resetea a 0
            0xFF04 => self.div = 0,
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value,
            // DMA Transfer: copia 160 bytes desde (value << 8) hacia OAM (0xFE00-0xFE9F)
            // Esto permite transferir datos de sprites rápidamente
            0xFF46 => {
                let base = (value as u16) << 8;
                for i in 0..0xA0u16 {
                    let b = self.read(base + i);
                    self.oam[i as usize] = b;
                }
            }
            // Escribir a LY lo resetea (comportamiento real de la GBC)
            0xFF44 => self.linea_lcd = 0,
            0xFF45 => self.lyc = value,
            // Solo los bits 3-7 del STAT son escribibles; los bits 0-2 los controla el PPU
            0xFF41 => self.stat = (self.stat & 0x07) | (value & 0xF8),
            _ => {} // Registros no implementados — ignorar escrituras
        }
    }

    // Incrementa la línea LCD actual. Se llama desde el PPU cada 456 ciclos.
    // Las líneas 0-143 son visibles, 144-153 son VBlank.
    pub fn tick_linea(&mut self) {
        self.linea_lcd = self.linea_lcd.wrapping_add(1);
        if self.linea_lcd > 153 {
            self.linea_lcd = 0;
        }
    }

    // Actualiza los registros del timer según los ciclos transcurridos.
    // DIV se incrementa a 16384 Hz (cada 256 ciclos de CPU a 4MHz).
    // TIMA se incrementa según la frecuencia configurada en TAC.
    // Cuando TIMA desborda, se resetea a TMA y se dispara una interrupción de timer.
    pub fn tick_timer(&mut self, ciclos: u8) {
        // DIV: se incrementa cada 256 ciclos
        self.ciclos_div += ciclos as u32;
        if self.ciclos_div >= 256 {
            self.ciclos_div -= 256;
            self.div = self.div.wrapping_add(1);
        }

        // Si el bit 2 de TAC es 0, el timer está desactivado
        if self.tac & 0x04 == 0 {
            return;
        }

        // Frecuencia del timer según bits 0-1 de TAC
        let frecuencia = match self.tac & 0x03 {
            0 => 1024, // 4096 Hz
            1 => 16,   // 262144 Hz
            2 => 64,   // 65536 Hz
            _ => 256,  // 16384 Hz
        };

        self.ciclos_timer += ciclos as u32;
        if self.ciclos_timer >= frecuencia {
            self.ciclos_timer -= frecuencia;
            let (nuevo, overflow) = self.tima.overflowing_add(1);
            if overflow {
                self.tima = self.tma; // resetear al valor base
                self.if_ |= 0x04; // activar interrupción de timer (bit 2)
            } else {
                self.tima = nuevo;
            }
        }
    }

    // Manejo de botones del Joypad
    pub fn key_down(&mut self, btn: JoypadButton) {
        let antes = match btn {
            JoypadButton::Right | JoypadButton::Left | JoypadButton::Up | JoypadButton::Down => {
                self.botones_dir
            }
            JoypadButton::A | JoypadButton::B | JoypadButton::Select | JoypadButton::Start => {
                self.botones_accion
            }
        };

        match btn {
            JoypadButton::Right => self.botones_dir &= !(1 << 0),
            JoypadButton::Left => self.botones_dir &= !(1 << 1),
            JoypadButton::Up => self.botones_dir &= !(1 << 2),
            JoypadButton::Down => self.botones_dir &= !(1 << 3),
            JoypadButton::A => self.botones_accion &= !(1 << 0),
            JoypadButton::B => self.botones_accion &= !(1 << 1),
            JoypadButton::Select => self.botones_accion &= !(1 << 2),
            JoypadButton::Start => self.botones_accion &= !(1 << 3),
        }

        let despues = match btn {
            JoypadButton::Right | JoypadButton::Left | JoypadButton::Up | JoypadButton::Down => {
                self.botones_dir
            }
            JoypadButton::A | JoypadButton::B | JoypadButton::Select | JoypadButton::Start => {
                self.botones_accion
            }
        };

        // Si pasó de no presionado (1) a presionado (0), solicitar interrupción de Joypad (bit 4 de IF / 0x10)
        if antes != despues {
            self.if_ |= 0x10;
        }
    }

    pub fn key_up(&mut self, btn: JoypadButton) {
        match btn {
            JoypadButton::Right => self.botones_dir |= 1 << 0,
            JoypadButton::Left => self.botones_dir |= 1 << 1,
            JoypadButton::Up => self.botones_dir |= 1 << 2,
            JoypadButton::Down => self.botones_dir |= 1 << 3,
            JoypadButton::A => self.botones_accion |= 1 << 0,
            JoypadButton::B => self.botones_accion |= 1 << 1,
            JoypadButton::Select => self.botones_accion |= 1 << 2,
            JoypadButton::Start => self.botones_accion |= 1 << 3,
        }
    }

    pub fn title(&self) -> String {
        self.cartridge.title()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoypadButton {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

