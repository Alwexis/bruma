mod apu;
mod cartridge;
mod cpu;
mod mmu;
mod ppu;

use apu::APU;
use cartridge::Cartridge;
use cpu::CPU;
use mmu::{JoypadButton, MMU};
use ppu::PPU;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use pixels::{Pixels, SurfaceTexture};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{env, path::Path};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() {
    // Seleccionar ROM: primero argumento de línea de comandos, luego buscar por defecto
    let rom_path = seleccionar_rom();
    let cart = Cartridge::load(&rom_path).expect("No se pudo cargar la ROM");

    // Buffer circular de audio para streaming estéreo
    let audio_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(16384)));

    // Inicializar dispositivo de audio cpal con configuración nativa
    let host = cpal::default_host();
    let device = host.default_output_device();

    let (_stream, sample_rate) = if let Some(dev) = device {
        let dev_name = dev.name().unwrap_or_else(|_| "Predeterminado".into());
        match dev.default_output_config() {
            Ok(default_config) => {
                let sample_rate = default_config.sample_rate().0;
                let channels = default_config.channels() as usize;
                let sample_format = default_config.sample_format();
                let config: cpal::StreamConfig = default_config.into();
                let err_fn = |err| eprintln!("Error en stream de audio: {err}");

                let buf_clone = Arc::clone(&audio_buffer);
                let stream_res = match sample_format {
                    cpal::SampleFormat::F32 => dev.build_output_stream(
                        &config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            if let Ok(mut buf) = buf_clone.lock() {
                                for frame in data.chunks_mut(channels) {
                                    let l = buf.pop_front().unwrap_or(0.0);
                                    let r = buf.pop_front().unwrap_or(l);
                                    if channels >= 2 {
                                        frame[0] = l;
                                        frame[1] = r;
                                        for extra in frame[2..].iter_mut() {
                                            *extra = 0.0;
                                        }
                                    } else if channels == 1 {
                                        frame[0] = (l + r) * 0.5;
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    ),
                    cpal::SampleFormat::I16 => dev.build_output_stream(
                        &config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            if let Ok(mut buf) = buf_clone.lock() {
                                for frame in data.chunks_mut(channels) {
                                    let l = buf.pop_front().unwrap_or(0.0);
                                    let r = buf.pop_front().unwrap_or(l);
                                    let l_i16 = (l.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                    let r_i16 = (r.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                    if channels >= 2 {
                                        frame[0] = l_i16;
                                        frame[1] = r_i16;
                                        for extra in frame[2..].iter_mut() {
                                            *extra = 0;
                                        }
                                    } else if channels == 1 {
                                        frame[0] = ((l + r) * 0.5 * i16::MAX as f32) as i16;
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    ),
                    cpal::SampleFormat::U16 => dev.build_output_stream(
                        &config,
                        move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                            if let Ok(mut buf) = buf_clone.lock() {
                                for frame in data.chunks_mut(channels) {
                                    let l = buf.pop_front().unwrap_or(0.0);
                                    let r = buf.pop_front().unwrap_or(l);
                                    let l_u16 = ((l.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
                                    let r_u16 = ((r.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
                                    if channels >= 2 {
                                        frame[0] = l_u16;
                                        frame[1] = r_u16;
                                        for extra in frame[2..].iter_mut() {
                                            *extra = u16::MAX / 2;
                                        }
                                    } else if channels == 1 {
                                        frame[0] = (((l + r) * 0.25 + 0.5) * u16::MAX as f32) as u16;
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    ),
                    _ => {
                        eprintln!("Formato de audio no soportado: {sample_format:?}");
                        Err(cpal::BuildStreamError::DeviceNotAvailable)
                    }
                };

                match stream_res {
                    Ok(s) => {
                        if let Err(e) = s.play() {
                            eprintln!("No se pudo iniciar el stream: {e}");
                            (None, 44100)
                        } else {
                            println!("Audio iniciado en '{}' ({} Hz, {} canales)", dev_name, sample_rate, channels);
                            (Some(s), sample_rate)
                        }
                    }
                    Err(e) => {
                        eprintln!("Error al construir stream de audio: {e}");
                        (None, 44100)
                    }
                }
            }
            Err(e) => {
                eprintln!("Error al obtener configuración de audio: {e}");
                (None, 44100)
            }
        }
    } else {
        eprintln!("No se encontró dispositivo de audio.");
        (None, 44100)
    };

    let apu = APU::new(sample_rate, audio_buffer);
    let mut mmu = MMU::new(cart, apu);
    let mut cpu = CPU::new();
    let mut ppu = PPU::new();

    let rom_title = mmu.title();
    println!("ROM: {}", rom_path);
    println!("Título: {}", if rom_title.is_empty() { "Desconocido" } else { &rom_title });
    println!("Tipo cartucho: {:#04x}", mmu.read(0x0147));
    println!("\n=== Controles de Teclado ===");
    println!("  D-Pad:     Flechas / W A S D");
    println!("  Botón A:   Z / J");
    println!("  Botón B:   X / K");
    println!("  Start:     Enter");
    println!("  Select:    Espacio / Shift / Retroceso");
    println!("  Salir:     Escape\n");

    let event_loop = EventLoop::new();
    let title = if rom_title.is_empty() {
        "Bruma".to_string()
    } else {
        format!("Bruma - {}", rom_title)
    };

    let window = WindowBuilder::new()
        .with_title(title)
        // La GBC tiene pantalla de 160x144 — la escalamos 3x para que sea visible
        .with_inner_size(LogicalSize::new(160u32 * 3, 144u32 * 3))
        .build(&event_loop)
        .unwrap();

    let mut pixels = {
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, &window);
        // El framebuffer interno es siempre 160x144, pixels lo escala a la ventana
        Pixels::new(160, 144, surface).unwrap()
    };

    // La GBC corre a exactamente 4.194304 MHz con 70224 ciclos por frame → ~59.7 fps
    let duracion_frame = Duration::from_nanos(16_742_706);
    let mut ultimo_frame = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                control_flow.set_exit();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    eprintln!("Error al redimensionar la superficie: {err}");
                }
            }
            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { new_inner_size, .. },
                ..
            } => {
                if let Err(err) = pixels.resize_surface(new_inner_size.width, new_inner_size.height) {
                    eprintln!("Error al redimensionar la superficie: {err}");
                }
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput {
                    input: KeyboardInput {
                        state,
                        virtual_keycode: Some(key),
                        ..
                    },
                    ..
                },
                ..
            } => {
                if key == VirtualKeyCode::Escape {
                    control_flow.set_exit();
                    return;
                }

                let button = match key {
                    VirtualKeyCode::Right | VirtualKeyCode::D => Some(JoypadButton::Right),
                    VirtualKeyCode::Left | VirtualKeyCode::A => Some(JoypadButton::Left),
                    VirtualKeyCode::Up | VirtualKeyCode::W => Some(JoypadButton::Up),
                    VirtualKeyCode::Down | VirtualKeyCode::S => Some(JoypadButton::Down),
                    VirtualKeyCode::Z | VirtualKeyCode::J => Some(JoypadButton::A),
                    VirtualKeyCode::X | VirtualKeyCode::K => Some(JoypadButton::B),
                    VirtualKeyCode::Return => Some(JoypadButton::Start),
                    VirtualKeyCode::Space
                    | VirtualKeyCode::Back
                    | VirtualKeyCode::RShift
                    | VirtualKeyCode::LShift => Some(JoypadButton::Select),
                    _ => None,
                };

                if let Some(btn) = button {
                    match state {
                        ElementState::Pressed => mmu.key_down(btn),
                        ElementState::Released => mmu.key_up(btn),
                    }
                }
            }
            Event::MainEventsCleared => {
                let ahora = Instant::now();
                // Acumulador de tiempo preciso para 59.7275 fps
                while ahora.duration_since(ultimo_frame) >= duracion_frame {
                    ultimo_frame += duracion_frame;
                    // Evitar espiral si la ventana estuvo pausada/minimizada
                    if ahora.duration_since(ultimo_frame) > duracion_frame * 5 {
                        ultimo_frame = ahora;
                    }

                    let frame = pixels.frame_mut();
                    let mut ciclos_frame = 0u32;
                    // Ejecutar exactamente 70224 ciclos por frame
                    while ciclos_frame < 70224 {
                        // Primero verificar si hay interrupciones pendientes
                        cpu.handle_interrupts(&mut mmu);
                        // Ejecutar una instrucción y obtener cuántos ciclos tomó
                        let ciclos = cpu.step(&mut mmu) as u32;
                        ciclos_frame += ciclos;
                        // Actualizar el timer con los ciclos transcurridos
                        mmu.tick_timer(ciclos as u8);
                        // Avanzar la APU de audio con los ciclos transcurridos
                        mmu.apu.tick(ciclos as u8);
                        // Avanzar el PPU el mismo número de ciclos
                        ppu.step(ciclos as u8, frame, &mut mmu);
                    }
                    mmu.apu.flush_samples();
                    pixels.render().unwrap();
                }
            }
            _ => {}
        }
    });
}

fn seleccionar_rom() -> String {
    if let Some(arg) = env::args().nth(1) {
        return arg;
    }

    let preferidas = ["roms/pokemon_amarillo.gbc", "roms/pokemon_cristal.gbc"];

    for path in preferidas {
        if Path::new(path).exists() {
            return path.to_string();
        }
    }

    "roms/pokemon_amarillo.gbc".to_string()
}
