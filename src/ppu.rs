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
  ciclos: u32 // contados de ciclos
}

impl PPU {
  pub fn new() -> PPU {
    PPU {
      framebuffer: vec![0u8; 160 * 144 * 4],
      ciclos: 0,
    }
  }

  pub fn step(&mut self, ciclos: u8, buffer: &mut [u8]) {
    self.ciclos += ciclos as u32;

    if self.ciclos >= 456 {
      self.ciclos -= 456;

      for pixel in buffer.chunks_mut(4) {
        pixel[0] = 0x00;
        pixel[1] = 0xFF;
        pixel[2] = 0x00;
        pixel[3] = 0xFF;
      }
    }
  }
}