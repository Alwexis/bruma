mod cartridge;
mod mmu;
mod cpu;
mod ppu;

use cartridge::Cartridge;
use mmu::MMU;
use cpu::CPU;
use ppu::PPU;

use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

fn main() {
    let cart = Cartridge::load("roms/pokemon_cristal.gbc").expect("No se pudo cargar la ROM");
    let mut mmu = MMU::new(cart);
    let mut cpu = CPU::new();
    let mut ppu = PPU::new();

    // el event loop es el corazon de la app. Escucha eventos del OS.
    let event_loop = EventLoop::new();
    // construye la ventana con config encadenada, en nuestro caso es 160*3 y 144*# que es la resolucion de gbc escalada 3x
    let window = WindowBuilder::new()
        .with_title("Bruma")
        .with_inner_size(LogicalSize::new(160u32 * 3, 144u32 * 3))
        .build(&event_loop)
        .unwrap();
    
    let mut pixels = {
        let size = window.inner_size();
        // surface texture conecta el framebuffer con la ventana
        let surface = SurfaceTexture::new(size.width, size.height, &window);
        // este crea un framebuffer de 160x144 (el tamaño real de la pantalla del gbc)
        Pixels::new(160, 144, surface).unwrap()
    };

    event_loop.run(move |event, _, control_flow| {
        // le dice al event loop que corra lo mas rapido posible sin esperar eventos
        control_flow.set_poll();

        match event {
            // cuando el user cierra la ventana, el set_exit() termina el programa de forma limpia
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                control_flow.set_exit();
            }
            // se ejecuta una vez por frame, aca es donde corre el emulador. El 70224 es exactamente un frame completo de la GBC
            // 456 ciclos * 154 lineas. Primero corre la CPU y el PPU, luego pixels.render manda el framebuffer a la pantalla.
            Event::MainEventsCleared => {
                for _ in 0..70224 {
                    let ciclos = cpu.step(&mut mmu);
                    ppu.step(ciclos, pixels.frame_mut(), &mut mmu);
                }
                pixels.render().unwrap();
            }
            _ => {}
        }
    });
}