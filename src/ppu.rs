use crate::mmu::MMU;

pub struct PPU {
    ciclos: u32,
    linea_actual: u8,
    lcdc_snapshot: u8,
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            ciclos: 0,
            linea_actual: 0,
            lcdc_snapshot: 0x91,
        }
    }

    pub fn step(&mut self, ciclos: u8, buffer: &mut [u8], mmu: &mut MMU) {
        if mmu.lcdc & 0b10000000 != 0 {
            self.lcdc_snapshot = mmu.lcdc;
        }

        self.ciclos += ciclos as u32;

        if self.ciclos >= 456 {
            mmu.tick_linea();
            self.ciclos -= 456;
            self.linea_actual += 1;

            if self.linea_actual < 144 {
                // forzamos modo 0x8000 y mapa 0x9800 para debug
                let mapa_base = 0x9800u16;
                let tile_y = self.linea_actual / 8;
                let linea_en_tile = self.linea_actual % 8;
                for tile_x in 0..20u8 {
                    let tile_index = mmu.read(mapa_base + tile_y as u16 * 32 + tile_x as u16);
                    self.dibujar_linea(buffer, tile_x, tile_y, linea_en_tile, tile_index, mmu);
                }
            } else if self.linea_actual == 144 {
                mmu.if_ |= 0x01;
            } else if self.linea_actual >= 154 {
                self.linea_actual = 0;
                mmu.if_ &= !0x01;
            }
        }
    }

    fn color_de_paleta(valor: u8, paleta: u8) -> [u8; 4] {
        let color_id = (paleta >> (valor * 2)) & 0x03;
        match color_id {
            0 => [0xFF, 0xFF, 0xFF, 0xFF],
            1 => [0xAA, 0xAA, 0xAA, 0xFF],
            2 => [0x55, 0x55, 0x55, 0xFF],
            3 => [0x00, 0x00, 0x00, 0xFF],
            _ => [0xFF, 0x00, 0xFF, 0xFF],
        }
    }

    fn dibujar_linea(
        &self,
        buffer: &mut [u8],
        tile_x: u8,
        tile_y: u8,
        linea: u8,
        tile_index: u8,
        mmu: &MMU,
    ) {
        // forzamos modo 0x8000 — índice sin signo siempre
        let tile_addr = 0x8000u16 + (tile_index as u16 * 16);

        let byte1 = mmu.read(tile_addr + linea as u16 * 2);
        let byte2 = mmu.read(tile_addr + linea as u16 * 2 + 1);

        for bit in 0..8u8 {
            let bit_bajo = (byte1 >> (7 - bit)) & 1;
            let bit_alto = (byte2 >> (7 - bit)) & 1;
            let valor = (bit_alto << 1) | bit_bajo;

            let px = tile_x as u32 * 8 + bit as u32;
            let py = tile_y as u32 * 8 + linea as u32;
            let indice = (py * 160 + px) as usize * 4;

            // usamos paleta fija 0xE4 para debug
            let color = Self::color_de_paleta(valor, 0xE4);
            buffer[indice..indice + 4].copy_from_slice(&color);
        }
    }
}