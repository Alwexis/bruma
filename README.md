Bruma es un emulador de GameboyColor escrito en Rust

Está hecho con fines educativos, para entender mejor cómo funcionan el hardware y software de las GBC y también para aprender mejor Rust.

## Controles de Teclado

| Botón Game Boy      | Teclas en Teclado                 |
| ------------------- | --------------------------------- |
| **D-Pad Arriba**    | `Flecha Arriba` / `W`             |
| **D-Pad Abajo**     | `Flecha Abajo` / `S`              |
| **D-Pad Izquierda** | `Flecha Izquierda` / `A`          |
| **D-Pad Derecha**   | `Flecha Derecha` / `D`            |
| **Botón A**         | `Z` / `J`                         |
| **Botón B**         | `X` / `K`                         |
| **Start**           | `Enter`                           |
| **Select**          | `Espacio` / `Shift` / `Retroceso` |
| **Salir**           | `Escape`                          |

## Ejecución

```bash
cargo run -- [ruta_a_rom.gbc]
```

## Referencias

[Pan Docs](https://gbdev.io/pandocs/): La "Biblia Técnica" del Gameboy
[GBDocs](https://gbdev.io/gb-opcodes/optables/): Documentaciones de GameBoy; el link anclado es para el set de instrucciones. Ej de cómo buscar, si PC es 0xFE debes buscar Row F y Column E (CP A, n8)


## Showcase
<div align="center">
  <img width="48%" height="480" alt="Bomberman" src="https://github.com/user-attachments/assets/ccd7b518-1ee0-4b99-93bf-286e02aa2333" />
  <img width="48%" height="480" alt="Megaman 5" src="https://github.com/user-attachments/assets/498f40f3-6b2e-428b-abc5-7007a2e9553e" />
</div>
