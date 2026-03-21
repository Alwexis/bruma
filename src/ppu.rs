use crate::mmu::MMU;
/*
El PPU (Picture Processing Unit) es el chip de la GBC que se encarga de generar la imagen. Igual que la MMU es intermediaria
entre la cpu y la memoria, el PPU es el intermediario entre la memoria de video y la pantalla.

la pantalla de la gbc tiene 160x144 pixeles. El PPU dibuja linea por linea de arriba hacia abajo, 60 veces pors egundo.

Como funciona:
la GBC no dibuja pixeles indiivduales, dibuja TILES, un tile es un bloque de 8x8 px. La pantalla entera es una grilla de
28x18 tiles. Los tiles estan guardados en la VRAM, la memoria de video entre 0x8000 y 0x9FFF. El PPU lee esos tiles y los
compone en pantalla
*/

pub struct PPU {
  framebuffer: Vec<u8>, // buffer de pixels, tamaño 160 * 144 * 4 (4 bytes por pixel RGBA)
  ciclos: u32, // contados de ciclos
  vram: Vec<u8>,
  linea_actual: u8
}

impl PPU {
  pub fn new() -> PPU {
    PPU {
      framebuffer: vec![0u8; 160 * 144 * 4],
      ciclos: 0,
      vram: vec![0u8; 0x2000],
      linea_actual: 0
    }
  }

  pub fn step(&mut self, ciclos: u8, buffer: &mut [u8], mmu: &mut MMU) {
    self.ciclos += ciclos as u32;

    if self.ciclos >= 456 {
      mmu.tick_linea();
      self.ciclos -= 456;
      self.linea_actual += 1;

      if self.linea_actual == 144 {
        for tile_y in 0..18u8 {
          for tile_x in 0..20u8 {
            let tile_index = mmu.read(0x9800 + tile_y as u16 * 32 + tile_x as u16);
            self.dibujar_tile(buffer, tile_x, tile_y, tile_index, mmu);
          }
        }
      } else if self.linea_actual >= 154 {
        self.linea_actual = 0
      }
    }
  }

  // convierte un valor de 0-3 a un color RGBA. Los 4 valores posbiles de la GBC mapeados a blanco, gris claro, gris oscuro y negro.
  // el magenta indica un error, nunca deberia aparecer.
  fn color_para_valor(valor: u8) -> [u8; 4] {
    match valor {
      0 => [0xFF, 0xFF, 0xFF, 0xFF], // blanco
      1 => [0xAA, 0xAA, 0xAA, 0xFF], // gris claro
      2 => [0x55, 0x55, 0x55, 0xFF], // gris oscuro
      3 => [0x00, 0x00, 0x00, 0xFF], // negro
      _ => [0xFF, 0x00, 0xFF, 0xFF], // magenta - error
    }
  }

  fn dibujar_tile(&self, buffer: &mut [u8], tile_x: u8, tile_y: u8, tile_index: u8, mmu: &MMU) {
    // direccion en la vram donde empieza el tile. Los tiles arrancan en 0x8000 y cada uno ocupa 16 bytes entonces el
    // el tile N esta en 0x8000 + N * 16.
    let tile_addr = 0x8000u16 + (tile_index as u16 * 16);

    for linea in 0..8u8 {
      // cada linea horizotnal del tile ocupa 2 bytes consecutivos. Se multiplica linea * 2 porque cada linea ocupa 2 bytes.
      // byte1 tiene los bits bajos del color y byte2 los bits altos.
      let byte1 = mmu.read(tile_addr + linea as u16 * 2);
      let byte2 = mmu.read(tile_addr + linea as u16 * 2 + 1);

      for bit in 0..8u8 {
        // se usa 7 - bit porque los pixeles van de izquierda a derecha, pero los bits de un byte van de derecha a izquierda
        // el & 1 aisla solo ese bit dando 0 o 1
        let bit_bajo = (byte1 >> (7 - bit)) & 1;
        let bit_alto = (byte2 >> (7 - bit)) & 1;
        // se arma un numero de 2 bits combinando el bit_alto y bit_bajo.
        // bit_aot << 1 lo pone en la posición 1 y | bit_bajo mete el bit bajo en la posicion 0. Resultando entre 0 y 3
        let valor = (bit_alto << 1) | bit_bajo;

        // coordenadas absolutas del pixel en la pantalla.
        // tile_x * 8 es la columna donde empieza el tile y + bit avanza dentro del tile. Mismo concepto en vertical con
        // tile_y y linea.
        let px = tile_x as u32 * 8 + bit as u32;
        let py = tile_y as u32 * 8 + linea as u32;
        // convierte coordenadas 2D a indice 1D en el framebuffer. py * 160 salta las filas anteriores (160 pixels de ancho)
        // y + px avanza en la fila actual. Se multiplica por 4 porque cada pixel ocupa 4 bytes RGBA.
        let indice = (py * 160 + px) as usize * 4;

        let color = Self::color_para_valor(valor);
        // el copy_from_slice escribe los 4 bytes del color en el framebuffer en la posición del pixel actual
        buffer[indice..indice + 4].copy_from_slice(&color);
      }
    }
  }
}