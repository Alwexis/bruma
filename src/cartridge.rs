// Tipos de mapper (controlador de memoria del cartucho).
// El mapper determina cómo se accede a los bancos de ROM y RAM del cartucho.
#[derive(Clone, Copy)]
enum Mapper {
    RomOnly, // Sin banking, ROM simple de hasta 32KB
    Mbc1,    // Memory Bank Controller 1 — Pokémon Rojo/Azul
    Mbc3,    // Memory Bank Controller 3 + RTC — Pokémon Cristal
    Mbc5,    // Memory Bank Controller 5 — Pokémon Amarillo
}

pub struct Cartridge {
    data: Vec<u8>,        // datos completos de la ROM
    mapper: Mapper,       // tipo de mapper detectado del header
    banco_rom: u16,       // banco de ROM actualmente seleccionado
    banco_ram: u8,        // banco de RAM externa actualmente seleccionado
    ram_habilitada: bool, // si la RAM externa está habilitada para escritura
    ram_externa: Vec<u8>, // RAM externa del cartucho (save data)
    // MBC1 específico
    mbc1_modo: u8,       // 0 = modo ROM (default), 1 = modo RAM
    mbc1_banco_alto: u8, // bits 5-6 del banco de ROM (o banco RAM en modo 1)
}

impl Cartridge {
    pub fn load(path: &str) -> Result<Cartridge, std::io::Error> {
        let data = std::fs::read(path)?;

        // El byte 0x0147 del header indica el tipo de cartucho/mapper
        let cart_type = data.get(0x0147).copied().unwrap_or(0x00);
        let mapper = match cart_type {
            0x00 => Mapper::RomOnly,
            0x01..=0x03 => Mapper::Mbc1,
            0x0F..=0x13 => Mapper::Mbc3,
            0x19..=0x1E => Mapper::Mbc5,
            _ => Mapper::RomOnly,
        };

        // El byte 0x0149 indica el tamaño de la RAM externa
        let ram_size_code = data.get(0x0149).copied().unwrap_or(0x00);
        let ram_size = match ram_size_code {
            0x00 => 0,
            0x01 => 2 * 1024,
            0x02 => 8 * 1024,
            0x03 => 32 * 1024,
            0x04 => 128 * 1024,
            0x05 => 64 * 1024,
            _ => 0,
        };

        Ok(Cartridge {
            data,
            mapper,
            banco_rom: 1,
            banco_ram: 0,
            ram_habilitada: false,
            // garantizamos al menos 32KB de RAM externa para evitar panics
            ram_externa: vec![0; ram_size.max(0x8000)],
            mbc1_modo: 0,
            mbc1_banco_alto: 0,
        })
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            // Banco 0 fijo (0x0000-0x3FFF)
            // En MBC1 modo RAM, el banco alto puede afectar este rango
            0x0000..=0x3FFF => {
                let banco = match self.mapper {
                    Mapper::Mbc1 if self.mbc1_modo == 1 => (self.mbc1_banco_alto as usize) << 5,
                    _ => 0,
                };
                let offset = banco * 0x4000 + address as usize;
                if offset < self.data.len() {
                    self.data[offset]
                } else {
                    0xFF
                }
            }
            // Banco switcheable (0x4000-0x7FFF)
            // El mapper selecciona qué banco de ROM mapear aquí
            0x4000..=0x7FFF => {
                let total_bancos = (self.data.len() / 0x4000).max(1);
                let banco = match self.mapper {
                    Mapper::RomOnly => 1usize.min(total_bancos.saturating_sub(1)),
                    Mapper::Mbc1 => {
                        // MBC1: combina bits bajos (banco_rom) con bits altos (mbc1_banco_alto)
                        let bajo = (self.banco_rom as usize) & 0x1F;
                        let alto = (self.mbc1_banco_alto as usize) << 5;
                        (alto | bajo) % total_bancos
                    }
                    Mapper::Mbc3 => (self.banco_rom as usize) % total_bancos,
                    Mapper::Mbc5 => (self.banco_rom as usize) % total_bancos,
                };
                let offset = banco * 0x4000 + (address as usize - 0x4000);
                if offset < self.data.len() {
                    self.data[offset]
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    // El juego escribe a la ROM para controlar el mapper.
    // Paradójico pero así funciona: escribir a 0x0000-0x7FFF no modifica la ROM,
    // sino que configura registros internos del mapper.
    pub fn write_mbc(&mut self, address: u16, value: u8) {
        match self.mapper {
            Mapper::RomOnly => {} // Sin mapper, ignorar escrituras
            Mapper::Mbc1 => match address {
                // 0x0000-0x1FFF: habilitar/deshabilitar RAM externa
                0x0000..=0x1FFF => {
                    self.ram_habilitada = (value & 0x0F) == 0x0A;
                }
                // 0x2000-0x3FFF: seleccionar banco ROM (bits bajos)
                // El banco 0 se remapea a 1 automáticamente
                0x2000..=0x3FFF => {
                    let v = (value & 0x1F) as u16;
                    self.banco_rom = if v == 0 { 1 } else { v };
                }
                // 0x4000-0x5FFF: bits altos del banco ROM (o banco RAM en modo 1)
                0x4000..=0x5FFF => {
                    self.mbc1_banco_alto = value & 0x03;
                }
                // 0x6000-0x7FFF: cambiar modo (0=ROM, 1=RAM)
                0x6000..=0x7FFF => {
                    self.mbc1_modo = value & 0x01;
                }
                _ => {}
            },
            Mapper::Mbc3 => match address {
                0x0000..=0x1FFF => {
                    self.ram_habilitada = (value & 0x0F) == 0x0A;
                }
                // MBC3 usa 7 bits para el banco ROM (hasta 128 bancos = 2MB)
                0x2000..=0x3FFF => {
                    let v = (value & 0x7F) as u16;
                    self.banco_rom = if v == 0 { 1 } else { v };
                }
                0x4000..=0x5FFF => {
                    self.banco_ram = value & 0x03;
                }
                _ => {}
            },
            Mapper::Mbc5 => match address {
                0x0000..=0x1FFF => {
                    self.ram_habilitada = (value & 0x0F) == 0x0A;
                }
                // MBC5 usa 9 bits para el banco ROM (hasta 512 bancos = 8MB)
                // Los 8 bits bajos van en 0x2000-0x2FFF
                0x2000..=0x2FFF => {
                    self.banco_rom = (self.banco_rom & 0x0100) | value as u16;
                }
                // El bit 9 va en 0x3000-0x3FFF
                0x3000..=0x3FFF => {
                    self.banco_rom = (self.banco_rom & 0x00FF) | (((value & 0x01) as u16) << 8);
                }
                0x4000..=0x5FFF => {
                    self.banco_ram = value & 0x0F;
                }
                _ => {}
            },
        }
    }

    pub fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_habilitada {
            return 0xFF;
        }
        // MBC3 puede mapear registros RTC (reloj en tiempo real) en lugar de RAM
        // cuando banco_ram >= 0x08. Por ahora devolvemos 0 como stub.
        if self.banco_ram >= 0x08 {
            return 0x00;
        }
        let bank_size = 0x2000usize;
        let bancos = (self.ram_externa.len() / bank_size).max(1);
        let banco = (self.banco_ram as usize) % bancos;
        let offset = banco * bank_size + (address as usize - 0xA000);
        self.ram_externa.get(offset).copied().unwrap_or(0xFF)
    }

    pub fn write_ram(&mut self, address: u16, value: u8) {
        if self.ram_externa.is_empty() || !self.ram_habilitada {
            return;
        }
        let bank_size = 0x2000usize;
        let bancos = (self.ram_externa.len() / bank_size).max(1);
        let banco = (self.banco_ram as usize) % bancos;
        let offset = banco * bank_size + (address as usize - 0xA000);
        if let Some(slot) = self.ram_externa.get_mut(offset) {
            *slot = value;
        }
    }

    // Lee el título del juego desde el header de la ROM (0x0134-0x0143)
    pub fn title(&self) -> String {
        if self.data.len() <= 0x0143 {
            return String::new();
        }
        let bytes = &self.data[0x0134..=0x0143];
        bytes
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect()
    }
}
