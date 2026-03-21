mod cartridge;
mod mmu;
mod cpu;

use cartridge::Cartridge;
use mmu::MMU;
use cpu::CPU;

fn main() {
    let cart = Cartridge::load("roms/pokemon_cristal.gbc");

    match cart {
        Ok(c) => {
            let mut mmu = MMU::new(c);
            let mut cpu = CPU::new();
            println!("ROM cargada correctamente");
            println!("Título: {:#?}", mmu.title());
            println!("Estado inicial CPU: {:#?}", cpu);
            for _ in 0..40 {
                cpu.step(&mut mmu);
                println!("PC: {:#06x}", cpu.pc);
            }
        },
        Err(e) => println!("Error al cargar la ROM: {}", e)
    }
}