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
            let mmu = MMU::new(c);
            let cpu = CPU::new();
            println!("ROM cargada correctamente");
            println!("Título: {:#?}", mmu.title());
            println!("Estado inicial CPU: {:#?}", cpu);
        },
        Err(e) => println!("Error al cargar la ROM: {}", e)
    }
}