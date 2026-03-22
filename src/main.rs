mod cartridge;
mod cpu;
mod mmu;
mod ppu;

use cartridge::Cartridge;
use cpu::CPU;
use mmu::MMU;
use ppu::PPU;

use pixels::{Pixels, SurfaceTexture};
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

fn main() {
    let cart = Cartridge::load("roms/pokemon_cristal.gbc").expect("No se pudo cargar la ROM");
    let mut mmu = MMU::new(cart);
    let mut cpu = CPU::new();
    let mut ppu = PPU::new();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Bruma")
        .with_inner_size(LogicalSize::new(160u32 * 3, 144u32 * 3))
        .build(&event_loop)
        .unwrap();

    let mut pixels = {
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, &window);
        Pixels::new(160, 144, surface).unwrap()
    };

    let mut ultimo_frame = Instant::now();
    let duracion_frame = Duration::from_nanos(16_742_706); // ~59.7fps

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                control_flow.set_exit();
            }
            Event::MainEventsCleared => {
                let ahora = Instant::now();
                if ahora - ultimo_frame >= duracion_frame {
                    ultimo_frame = ahora;
                    let mut ciclos_frame = 0u32;
                    while ciclos_frame < 70224 {
                        cpu.handle_interrupts(&mut mmu);
                        let ciclos = cpu.step(&mut mmu) as u32;
                        ciclos_frame += ciclos;
                        mmu.tick_timer(ciclos as u8);
                        ppu.step(ciclos as u8, pixels.frame_mut(), &mut mmu);
                    }
                    pixels.render().unwrap();
                }
            }
            _ => {}
        }
    });
}
