use crate::mmu::MMU;

/*
El PPU (Picture Processing Unit) es el chip que genera la imagen de la GBC.
Trabaja en conjunto con la CPU — mientras la CPU ejecuta lógica del juego,
el PPU dibuja la pantalla línea por línea en paralelo.

La pantalla tiene 160x144 píxeles. El PPU dibuja de arriba a abajo, 144 líneas
visibles, y luego entra en VBlank (líneas 144-153) donde el juego puede actualizar
la VRAM sin causar glitches visuales.

Cada línea toma exactamente 456 ciclos de CPU:
  - Modo 2 (OAM scan):  80 ciclos  — el PPU busca sprites en la línea actual
  - Modo 3 (Drawing):  172 ciclos  — el PPU dibuja la línea
  - Modo 0 (HBlank):   204 ciclos  — línea terminada, la CPU puede acceder a VRAM
  - (Modo 1 = VBlank, durante líneas 144-153)

Un frame completo = 154 líneas × 456 ciclos = 70224 ciclos ≈ 59.7 fps

Capas de la GBC:
  1. BG (Background): grilla de 32×32 tiles, con scroll
  2. Window: segunda capa de tiles sin scroll, encima del BG
  3. OBJ (Sprites): hasta 40 sprites, máximo 10 por línea
*/
pub struct PPU {
    ciclos: u32,         // ciclos acumulados en la línea actual
    lcdc_snapshot: u8,   // copia del LCDC del frame/línea
    frame_count: u32,    // contador de frames renderizados
    stat_irq_line: bool, // estado previo de la línea de interrupción STAT (para detección de flancos)
    window_line: u8,     // contador de línea interno para la ventana (se incrementa solo cuando la ventana se dibuja)
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            ciclos: 0,
            lcdc_snapshot: 0x91,
            frame_count: 0,
            stat_irq_line: false,
            window_line: 0,
        }
    }

    pub fn step(&mut self, ciclos: u8, buffer: &mut [u8], mmu: &mut MMU) {
        // Si el LCD está apagado (bit 7 del LCDC = 0), resetear todo y pantalla en blanco
        if mmu.lcdc & 0b1000_0000 == 0 {
            mmu.linea_lcd = 0;
            self.ciclos = 0;
            self.window_line = 0;
            mmu.stat = (mmu.stat & 0xF8) | 0x00;
            self.stat_irq_line = false;
            return;
        }

        self.lcdc_snapshot = mmu.lcdc;
        self.ciclos += ciclos as u32;

        // Cada 456 ciclos avanzamos una línea
        while self.ciclos >= 456 {
            self.ciclos -= 456;

            let ly = mmu.linea_lcd;

            // Dibujar línea visible actual ANTES de incrementar linea_lcd
            if ly < 144 {
                // Capa 1: fondo con scroll
                let mut bg_ids = self.dibujar_bg_linea(buffer, ly, mmu);

                // Capa 2: ventana (Window)
                // Bit 5 = Window Enable, Bit 0 = BG & Window Enable
                let win_enabled = (self.lcdc_snapshot & 0b0010_0000 != 0)
                    && (self.lcdc_snapshot & 0b0000_0001 != 0);
                if win_enabled && ly >= mmu.wy && mmu.wx <= 166 {
                    self.dibujar_window_linea(buffer, ly, self.window_line, mmu, &mut bg_ids);
                    self.window_line = self.window_line.wrapping_add(1);
                }

                // Capa 3: sprites (OBJ)
                // Bit 1 = OBJ Enable
                if self.lcdc_snapshot & 0b0000_0010 != 0 {
                    self.dibujar_obj_linea(buffer, ly, mmu, &bg_ids);
                }
            }

            // Incrementar línea LCD
            mmu.tick_linea();
            let nuevo_ly = mmu.linea_lcd;

            // Detectar inicio de nuevo frame
            if nuevo_ly == 0 {
                self.window_line = 0;
                self.frame_count = self.frame_count.wrapping_add(1);
            }

            // Al llegar a la línea 144 comienza VBlank — activar interrupción
            if nuevo_ly == 144 {
                mmu.if_ |= 0x01; // bit 0 = VBlank interrupt
            }
        }

        // Actualizar el modo del PPU en el registro STAT según la posición actual
        let ly = mmu.linea_lcd;
        let modo = if ly >= 144 {
            0x01 // Modo 1: VBlank
        } else if self.ciclos < 80 {
            0x02 // Modo 2: OAM scan
        } else if self.ciclos < 252 {
            0x03 // Modo 3: Drawing
        } else {
            0x00 // Modo 0: HBlank
        };

        // Actualizar bit de coincidencia LY==LYC en STAT (bit 2)
        if ly == mmu.lyc {
            mmu.stat |= 0x04;
        } else {
            mmu.stat &= !0x04;
        }

        mmu.stat = (mmu.stat & 0xFC) | modo;

        // Interrupción STAT — se dispara por eventos configurables
        let lyc_irq = (mmu.stat & 0x40) != 0 && (mmu.stat & 0x04) != 0; // LY==LYC
        let mode0_irq = (mmu.stat & 0x08) != 0 && modo == 0x00; // HBlank
        let mode1_irq = (mmu.stat & 0x10) != 0 && modo == 0x01; // VBlank
        let mode2_irq = (mmu.stat & 0x20) != 0 && modo == 0x02; // OAM scan
        let stat_line = lyc_irq || mode0_irq || mode1_irq || mode2_irq;

        // Solo disparar en flanco positivo (cuando la línea pasa de 0 a 1)
        if stat_line && !self.stat_irq_line {
            mmu.if_ |= 0x02; // bit 1 = LCD STAT interrupt
        }
        self.stat_irq_line = stat_line;
    }

    // Convierte un valor de color (0-3) a RGBA usando la paleta.
    // La paleta es un byte de 8 bits que codifica 4 colores de 2 bits cada uno:
    //   bits 1-0: color 0
    //   bits 3-2: color 1
    //   bits 5-4: color 2
    //   bits 7-6: color 3
    // Los valores de color se mapean a tonos verdes clásicos de Game Boy.
    fn color_de_paleta(valor: u8, paleta: u8) -> [u8; 4] {
        let color_id = (paleta >> (valor * 2)) & 0x03;
        match color_id {
            0 => [0xE0, 0xF8, 0xD0, 0xFF], // blanco verdoso
            1 => [0x88, 0xC0, 0x70, 0xFF], // verde claro
            2 => [0x34, 0x68, 0x56, 0xFF], // verde oscuro
            3 => [0x08, 0x18, 0x20, 0xFF], // casi negro
            _ => [0xFF, 0x00, 0xFF, 0xFF], // error
        }
    }

    // Dibuja una línea horizontal del fondo (BG layer).
    fn dibujar_bg_linea(&self, buffer: &mut [u8], ly: u8, mmu: &MMU) -> [u8; 160] {
        let mut bg_ids = [0u8; 160];

        // Si el BG está deshabilitado (bit 0 de LCDC = 0), rellenar con color 0
        if self.lcdc_snapshot & 0b0000_0001 == 0 {
            let color0 = Self::color_de_paleta(0, mmu.paleta_bg);
            for x in 0..160usize {
                let indice = (ly as usize * 160 + x) * 4;
                buffer[indice..indice + 4].copy_from_slice(&color0);
            }
            return bg_ids;
        }

        // El tile map del BG puede estar en 0x9800 o 0x9C00 según el bit 3 del LCDC
        let mapa_base = if self.lcdc_snapshot & 0b0000_1000 != 0 {
            0x9C00u16
        } else {
            0x9800u16
        };

        // Aplicar scroll: el BG puede desplazarse hasta 255 píxeles en X e Y
        let bg_y = ly.wrapping_add(mmu.scy);
        let tile_y = ((bg_y / 8) as u16) % 32;
        let linea_en_tile = (bg_y % 8) as u16;

        for x in 0..160u16 {
            let bg_x = (x as u8).wrapping_add(mmu.scx);
            let tile_x = ((bg_x / 8) as u16) % 32;
            let bit_en_tile = 7 - (bg_x % 8);

            let tile_index_addr = mapa_base + tile_y * 32 + tile_x;
            let tile_index = mmu.read(tile_index_addr);

            // Bit 4 del LCDC: modo 0x8000 (sin signo) o modo 0x9000 (con signo)
            let tile_addr = if self.lcdc_snapshot & 0b0001_0000 != 0 {
                0x8000u16 + (tile_index as u16 * 16)
            } else {
                let signed = tile_index as i8 as i32;
                (0x9000i32 + signed * 16) as u16
            };

            let byte1 = mmu.read(tile_addr + linea_en_tile * 2);
            let byte2 = mmu.read(tile_addr + linea_en_tile * 2 + 1);
            let bit_bajo = (byte1 >> bit_en_tile) & 1;
            let bit_alto = (byte2 >> bit_en_tile) & 1;
            let valor = (bit_alto << 1) | bit_bajo;
            bg_ids[x as usize] = valor;

            let indice = (ly as usize * 160 + x as usize) * 4;
            let color = Self::color_de_paleta(valor, mmu.paleta_bg);
            buffer[indice..indice + 4].copy_from_slice(&color);
        }

        bg_ids
    }

    // Dibuja una línea de la ventana (Window layer).
    fn dibujar_window_linea(
        &self,
        buffer: &mut [u8],
        ly: u8,
        win_y_actual: u8,
        mmu: &MMU,
        bg_ids: &mut [u8; 160],
    ) {
        let win_x0 = mmu.wx as i16 - 7;
        if win_x0 >= 160 {
            return;
        }

        // Tile map de la ventana: bit 6 del LCDC selecciona 0x9800 o 0x9C00
        let mapa_base = if self.lcdc_snapshot & 0b0100_0000 != 0 {
            0x9C00u16
        } else {
            0x9800u16
        };

        let tile_y = ((win_y_actual / 8) as u16) % 32;
        let linea_en_tile = (win_y_actual % 8) as u16;

        let x_inicio = if win_x0 < 0 { 0i16 } else { win_x0 };
        for x in x_inicio..160i16 {
            let win_x = (x - win_x0) as u16;
            let tile_x = (win_x / 8) % 32;
            let bit_en_tile = 7 - (win_x % 8);

            let tile_index_addr = mapa_base + tile_y * 32 + tile_x;
            let tile_index = mmu.read(tile_index_addr);

            let tile_addr = if self.lcdc_snapshot & 0b0001_0000 != 0 {
                0x8000u16 + (tile_index as u16 * 16)
            } else {
                let signed = tile_index as i8 as i32;
                (0x9000i32 + signed * 16) as u16
            };

            let byte1 = mmu.read(tile_addr + linea_en_tile * 2);
            let byte2 = mmu.read(tile_addr + linea_en_tile * 2 + 1);
            let bit_bajo = (byte1 >> bit_en_tile) & 1;
            let bit_alto = (byte2 >> bit_en_tile) & 1;
            let valor = (bit_alto << 1) | bit_bajo;

            bg_ids[x as usize] = valor;
            let indice = (ly as usize * 160 + x as usize) * 4;
            let color = Self::color_de_paleta(valor, mmu.paleta_bg);
            buffer[indice..indice + 4].copy_from_slice(&color);
        }
    }

    // Dibuja los sprites (OBJ) de una línea horizontal.
    // La GBC tiene 40 sprites en OAM, pero solo puede dibujar 10 por línea.
    // Cada sprite ocupa 4 bytes en OAM: Y, X, tile, atributos.
    fn dibujar_obj_linea(&self, buffer: &mut [u8], ly: u8, mmu: &MMU, bg_ids: &[u8; 160]) {
        // El bit 2 del LCDC determina el tamaño de los sprites: 0 = 8x8, 1 = 8x16
        let alto_obj = if self.lcdc_snapshot & 0b0000_0100 != 0 {
            16i16
        } else {
            8i16
        };

        let mut pixels_ocupados = [false; 160];
        let mut candidatos: Vec<(u8, u8, u16)> = Vec::with_capacity(10);

        // Encontrar hasta 10 sprites que intersecten la línea actual
        for i in 0..40u16 {
            if candidatos.len() >= 10 {
                break;
            }

            let base = 0xFE00 + i * 4;
            // Coordenada Y está desplazada 16 píxeles (sprite en Y=0 está fuera de pantalla)
            let y = mmu.read(base) as i16 - 16;
            let x_raw = mmu.read(base + 1);
            let ly_i = ly as i16;
            if ly_i < y || ly_i >= y + alto_obj {
                continue;
            }
            candidatos.push((x_raw, i as u8, base));
        }

        // Ordenar por X (y por índice como desempate) — los sprites con X menor
        // tienen prioridad y aparecen encima
        candidatos.sort_by_key(|(x, i, _)| (*x, *i));

        for (_, _, base) in candidatos {
            let y = mmu.read(base) as i16 - 16;
            // Coordenada X desplazada 8 píxeles
            let x = mmu.read(base + 1) as i16 - 8;
            let mut tile = mmu.read(base + 2);
            let attrs = mmu.read(base + 3);

            let ly_i = ly as i16;
            let mut linea = ly_i - y;

            // Bit 6 de attrs: flip vertical
            if attrs & 0b0100_0000 != 0 {
                linea = alto_obj - 1 - linea;
            }

            // En modo 8x16, el bit 0 del tile se ignora (siempre par)
            // y se selecciona el tile superior o inferior según la línea
            if alto_obj == 16 {
                tile &= 0xFE;
                if linea >= 8 {
                    tile = tile.wrapping_add(1);
                    linea -= 8;
                }
            }

            // Los sprites siempre usan el modo 0x8000 para sus tiles
            let tile_addr = 0x8000u16 + tile as u16 * 16;
            let byte1 = mmu.read(tile_addr + linea as u16 * 2);
            let byte2 = mmu.read(tile_addr + linea as u16 * 2 + 1);

            for p in 0..8i16 {
                let sx = x + p;
                if !(0..160).contains(&sx) {
                    continue;
                }
                let sx_usize = sx as usize;
                // Si ya hay un sprite dibujado en este pixel, respetar la prioridad
                if pixels_ocupados[sx_usize] {
                    continue;
                }

                // Bit 5 de attrs: flip horizontal
                let bit = if attrs & 0b0010_0000 != 0 {
                    p as u8
                } else {
                    7 - p as u8
                };
                let bit_bajo = (byte1 >> bit) & 1;
                let bit_alto = (byte2 >> bit) & 1;
                let valor = (bit_alto << 1) | bit_bajo;

                // El color 0 es siempre transparente en sprites
                if valor == 0 {
                    continue;
                }

                // Bit 7 de attrs: prioridad del BG
                // Si está activo y el BG tiene un color != 0, el BG aparece encima del sprite
                let prioridad_fondo = attrs & 0b1000_0000 != 0;
                if prioridad_fondo && bg_ids[sx_usize] != 0 {
                    continue;
                }

                // Bit 4 de attrs: seleccionar paleta de color (OBJ0 u OBJ1)
                let paleta = if attrs & 0b0001_0000 != 0 {
                    mmu.paleta_obj1
                } else {
                    mmu.paleta_obj0
                };

                let indice = (ly as usize * 160 + sx_usize) * 4;
                let color = Self::color_de_paleta(valor, paleta);
                buffer[indice..indice + 4].copy_from_slice(&color);
                pixels_ocupados[sx_usize] = true;
            }
        }
    }
}
