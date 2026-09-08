use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// patrones de ciclo de trabajo para ondas cuadradas
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25%
    [1, 0, 0, 0, 1, 1, 1, 1], // 50%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75%
];

// frecuencia de reloj de la gb
const CPU_CLOCK_HZ: u32 = 4_194_304;

pub struct APU {
    // frecuencia de muestreo configurada
    sample_rate: u32,

    // reigstros canal 1 (square y sweep)
    nr10: u8,
    nr11: u8,
    nr12: u8,
    nr13: u8,
    nr14: u8,
    ch1_enabled: bool,
    ch1_timer: u32,
    ch1_duty_step: usize,
    ch1_length_timer: u8,
    ch1_env_vol: u8,
    ch1_env_timer: u8,
    ch1_sweep_timer: u8,
    ch1_shadow_freq: u16,

    // registros canal 2 (square)
    nr21: u8,
    nr22: u8,
    nr23: u8,
    nr24: u8,
    ch2_enabled: bool,
    ch2_timer: u32,
    ch2_duty_step: usize,
    ch2_length_timer: u8,
    ch2_env_vol: u8,
    ch2_env_timer: u8,

    // registros del canal 3 (wave)
    nr30: u8,
    nr31: u8,
    nr32: u8,
    nr33: u8,
    nr34: u8,
    pub wave_ram: [u8; 16],
    ch3_enabled: bool,
    ch3_timer: u32,
    ch3_sample_pos: usize,
    ch3_length_timer: u16,

    // registros canal 4 (noise)
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,
    ch4_enabled: bool,
    ch4_timer: u32,
    ch4_lfsr: u16,
    ch4_length_timer: u8,
    ch4_env_vol: u8,
    ch4_env_timer: u8,

    // control global
    nr50: u8, // master volume / vin
    nr51: u8, // panning
    nr52: u8, // master enable

    // temporizadores
    frame_seq_timer: u32,
    frame_seq_step: u8,
    sample_timer: f64,
    // buffer local para batching y evitar contención de mutex
    local_buffer: Vec<f32>,
    // buffer compartido de audio (estéreo)
    pub audio_buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl APU {
    pub fn new(sample_rate: u32, audio_buffer: Arc<Mutex<VecDeque<f32>>>) -> Self {
        Self {
            sample_rate,
            nr10: 0x80,
            nr11: 0xBF,
            nr12: 0xF3,
            nr13: 0xFF,
            nr14: 0xBF,
            ch1_enabled: false,
            ch1_timer: 1,
            ch1_duty_step: 0,
            ch1_length_timer: 0,
            ch1_env_vol: 0,
            ch1_env_timer: 0,
            ch1_sweep_timer: 0,
            ch1_shadow_freq: 0,
            nr21: 0x3F,
            nr22: 0x00,
            nr23: 0xFF,
            nr24: 0xBF,
            ch2_enabled: false,
            ch2_timer: 1,
            ch2_duty_step: 0,
            ch2_length_timer: 0,
            ch2_env_vol: 0,
            ch2_env_timer: 0,
            nr30: 0x7F,
            nr31: 0xFF,
            nr32: 0x9F,
            nr33: 0xFF,
            nr34: 0xBF,
            wave_ram: [0; 16],
            ch3_enabled: false,
            ch3_timer: 1,
            ch3_sample_pos: 0,
            ch3_length_timer: 0,
            nr41: 0xFF,
            nr42: 0x00,
            nr43: 0x00,
            nr44: 0xBF,
            ch4_enabled: false,
            ch4_timer: 1,
            ch4_lfsr: 0x7FFF,
            ch4_length_timer: 0,
            ch4_env_vol: 0,
            ch4_env_timer: 0,
            nr50: 0x77,
            nr51: 0xF3,
            nr52: 0xF1,
            frame_seq_timer: 0,
            frame_seq_step: 0,
            sample_timer: 0.0,
            local_buffer: Vec::with_capacity(512),
            audio_buffer,
        }
    }

    pub fn tick(&mut self, cycles: u8) {
        if self.nr52 & 0x80 == 0 {
            return;
        }

        let c = cycles as u32;

        // frame squencer (512hz cada 8192 ciclos de cpu)
        self.frame_seq_timer += c;
        if self.frame_seq_timer >= 8192 {
            self.frame_seq_timer -= 8192;
            self.step_frame_sequencer();
        }

        // avanzamos canales
        self.step_ch1(c);
        self.step_ch2(c);
        self.step_ch3(c);
        self.step_ch4(c);

        // generar muestras a la frecuencia de muestreo del dispositivo
        self.sample_timer += c as f64;
        let cycles_per_sample = CPU_CLOCK_HZ as f64 / self.sample_rate as f64;
        while self.sample_timer >= cycles_per_sample {
            self.sample_timer -= cycles_per_sample;
            self.generate_sample();
        }
    }

    fn step_frame_sequencer(&mut self) {
        // step 0 length
        // step 2 length y sweep
        // step 4 lenght
        // step 6 length y sweep
        // step 7 volume envelope
        if self.frame_seq_step % 2 == 0 {
            self.step_length();
        }
        if self.frame_seq_step == 2 || self.frame_seq_step == 6 {
            self.step_sweep();
        }
        if self.frame_seq_step == 7 {
            self.step_envelope();
        }

        self.frame_seq_step = (self.frame_seq_step + 1) % 8;
    }

    fn step_length(&mut self) {
        // ch1
        if self.nr14 & 0x40 != 0 && self.ch1_length_timer > 0 {
            self.ch1_length_timer -= 1;
            if self.ch1_length_timer == 0 {
                self.ch1_enabled = false;
            }
        }
        // ch2
        if self.nr24 & 0x40 != 0 && self.ch2_length_timer > 0 {
            self.ch2_length_timer -= 1;
            if self.ch2_length_timer == 0 {
                self.ch2_enabled = false;
            }
        }
        // ch3
        if self.nr34 & 0x40 != 0 && self.ch3_length_timer > 0 {
            self.ch3_length_timer -= 1;
            if self.ch3_length_timer == 0 {
                self.ch3_enabled = false;
            }
        }
        // ch4
        if self.nr44 & 0x40 != 0 && self.ch4_length_timer > 0 {
            self.ch4_length_timer -= 1;
            if self.ch4_length_timer == 0 {
                self.ch4_enabled = false;
            }
        }
    }

    fn step_envelope(&mut self) {
        // ch1
        if self.nr12 & 0x07 != 0 {
            self.ch1_env_timer = self.ch1_env_timer.saturating_sub(1);
            if self.ch1_env_timer == 0 {
                self.ch1_env_timer = self.nr12 & 0x07;
                if self.nr12 & 0x08 != 0 {
                    if self.ch1_env_vol < 15 {
                        self.ch1_env_vol += 1;
                    }
                } else {
                    if self.ch1_env_vol > 0 {
                        self.ch1_env_vol -= 1;
                    }
                }
            }
        }
        // ch2
        if self.nr22 & 0x07 != 0 {
            self.ch2_env_timer = self.ch2_env_timer.saturating_sub(1);
            if self.ch2_env_timer == 0 {
                self.ch2_env_timer = self.nr22 & 0x07;
                if self.nr22 & 0x08 != 0 {
                    if self.ch2_env_vol < 15 {
                        self.ch2_env_vol += 1;
                    }
                } else {
                    if self.ch2_env_vol > 0 {
                        self.ch2_env_vol -= 1;
                    }
                }
            }
        }
        // ch4
        if self.nr42 & 0x07 != 0 {
            self.ch4_env_timer = self.ch4_env_timer.saturating_sub(1);
            if self.ch4_env_timer == 0 {
                self.ch4_env_timer = self.nr42 & 0x07;
                if self.nr42 & 0x08 != 0 {
                    if self.ch4_env_vol < 15 {
                        self.ch4_env_vol += 1;
                    }
                } else {
                    if self.ch4_env_vol > 0 {
                        self.ch4_env_vol -= 1;
                    }
                }
            }
        }
    }
    fn step_sweep(&mut self) {
        let pace = (self.nr10 >> 4) & 0x07;
        if pace == 0 {
            return;
        }
        if self.ch1_sweep_timer > 0 {
            self.ch1_sweep_timer -= 1;
        }
        if self.ch1_sweep_timer == 0 {
            self.ch1_sweep_timer = if pace == 0 { 8 } else { pace };
            let shift = self.nr10 & 0x07;
            if shift > 0 && self.ch1_enabled {
                let delta = self.ch1_shadow_freq >> shift;
                let new_freq = if self.nr10 & 0x08 != 0 {
                    self.ch1_shadow_freq.saturating_sub(delta)
                } else {
                    self.ch1_shadow_freq + delta
                };
                if new_freq <= 2047 {
                    self.ch1_shadow_freq = new_freq;
                    self.nr13 = (new_freq & 0xFF) as u8;
                    self.nr14 = (self.nr14 & 0xF8) | ((new_freq >> 8) as u8 & 0x07);
                } else {
                    self.ch1_enabled = false;
                }
            }
        }
    }
    fn step_ch1(&mut self, mut cycles: u32) {
        let freq = (self.nr13 as u32) | (((self.nr14 & 0x07) as u32) << 8);
        let period = (2048 - freq) * 4;
        while cycles >= self.ch1_timer {
            cycles -= self.ch1_timer;
            self.ch1_timer = if period == 0 { 4 } else { period };
            self.ch1_duty_step = (self.ch1_duty_step + 1) % 8;
        }
        self.ch1_timer -= cycles;
    }
    fn step_ch2(&mut self, mut cycles: u32) {
        let freq = (self.nr23 as u32) | (((self.nr24 & 0x07) as u32) << 8);
        let period = (2048 - freq) * 4;
        while cycles >= self.ch2_timer {
            cycles -= self.ch2_timer;
            self.ch2_timer = if period == 0 { 4 } else { period };
            self.ch2_duty_step = (self.ch2_duty_step + 1) % 8;
        }
        self.ch2_timer -= cycles;
    }
    fn step_ch3(&mut self, mut cycles: u32) {
        let freq = (self.nr33 as u32) | (((self.nr34 & 0x07) as u32) << 8);
        let period = (2048 - freq) * 2;
        while cycles >= self.ch3_timer {
            cycles -= self.ch3_timer;
            self.ch3_timer = if period == 0 { 2 } else { period };
            self.ch3_sample_pos = (self.ch3_sample_pos + 1) % 32;
        }
        self.ch3_timer -= cycles;
    }
    fn step_ch4(&mut self, mut cycles: u32) {
        let div_code = self.nr43 & 0x07;
        let divisor = if div_code == 0 {
            8
        } else {
            (div_code as u32) * 16
        };
        let shift = (self.nr43 >> 4) as u32;
        let period = divisor << shift;
        while cycles >= self.ch4_timer {
            cycles -= self.ch4_timer;
            self.ch4_timer = if period == 0 { 8 } else { period };
            let bit0 = self.ch4_lfsr & 1;
            let bit1 = (self.ch4_lfsr >> 1) & 1;
            let result = bit0 ^ bit1;
            self.ch4_lfsr = (self.ch4_lfsr >> 1) | (result << 14);
            if self.nr43 & 0x08 != 0 {
                self.ch4_lfsr = (self.ch4_lfsr & !0x40) | (result << 6);
            }
        }
        self.ch4_timer -= cycles;
    }
    fn generate_sample(&mut self) {
        // muestras de 0 a 15 de cada canal
        let mut sample1 = 0.0f32;
        if self.ch1_enabled && self.nr12 & 0xF8 != 0 {
            let duty = (self.nr11 >> 6) as usize;
            if DUTY_TABLE[duty][self.ch1_duty_step] == 1 {
                sample1 = self.ch1_env_vol as f32 / 15.0;
            }
        }
        let mut sample2 = 0.0f32;
        if self.ch2_enabled && self.nr22 & 0xF8 != 0 {
            let duty = (self.nr21 >> 6) as usize;
            if DUTY_TABLE[duty][self.ch2_duty_step] == 1 {
                sample2 = self.ch2_env_vol as f32 / 15.0;
            }
        }
        let mut sample3 = 0.0f32;
        if self.ch3_enabled && self.nr30 & 0x80 != 0 {
            let byte_idx = self.ch3_sample_pos / 2;
            let raw_sample = if self.ch3_sample_pos % 2 == 0 {
                self.wave_ram[byte_idx] >> 4
            } else {
                self.wave_ram[byte_idx] & 0x0F
            };
            let shift = match (self.nr32 >> 5) & 0x03 {
                0 => 4, // 0%
                1 => 0, // 100%
                2 => 1, // 50%
                _ => 2, // 25%
            };
            sample3 = (raw_sample >> shift) as f32 / 15.0;
        }
        let mut sample4 = 0.0f32;
        if self.ch4_enabled && self.nr42 & 0xF8 != 0 {
            if (self.ch4_lfsr & 1) == 0 {
                sample4 = self.ch4_env_vol as f32 / 15.0;
            }
        }
        // panning L/R (NR51)
        let mut left = 0.0f32;
        let mut right = 0.0f32;
        if self.nr51 & 0x01 != 0 {
            right += sample1;
        }
        if self.nr51 & 0x02 != 0 {
            right += sample2;
        }
        if self.nr51 & 0x04 != 0 {
            right += sample3;
        }
        if self.nr51 & 0x08 != 0 {
            right += sample4;
        }
        if self.nr51 & 0x10 != 0 {
            left += sample1;
        }
        if self.nr51 & 0x20 != 0 {
            left += sample2;
        }
        if self.nr51 & 0x40 != 0 {
            left += sample3;
        }
        if self.nr51 & 0x80 != 0 {
            left += sample4;
        }
        // volumen maestro (NR50)
        let vol_r = ((self.nr50 & 0x07) as f32 + 1.0) / 8.0;
        let vol_l = (((self.nr50 >> 4) & 0x07) as f32 + 1.0) / 8.0;
        left = (left / 4.0) * vol_l * 0.5;
        right = (right / 4.0) * vol_r * 0.5;

        // Acumular en buffer local
        self.local_buffer.push(left);
        self.local_buffer.push(right);
        if self.local_buffer.len() >= 256 {
            self.flush_samples();
        }
    }

    // Vacía el buffer local en el buffer compartido reduciendo la contención de mutex
    pub fn flush_samples(&mut self) {
        if self.local_buffer.is_empty() {
            return;
        }
        if let Ok(mut buf) = self.audio_buffer.lock() {
            if buf.len() < 16384 {
                for s in self.local_buffer.drain(..) {
                    buf.push_back(s);
                }
            } else {
                self.local_buffer.clear();
            }
        } else {
            self.local_buffer.clear();
        }
    }
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => self.nr10 | 0x80,
            0xFF11 => self.nr11 | 0x3F,
            0xFF12 => self.nr12,
            0xFF13 => 0xFF,
            0xFF14 => self.nr14 | 0xBF,
            0xFF16 => self.nr21 | 0x3F,
            0xFF17 => self.nr22,
            0xFF18 => 0xFF,
            0xFF19 => self.nr24 | 0xBF,
            0xFF1A => self.nr30 | 0x7F,
            0xFF1B => 0xFF,
            0xFF1C => self.nr32 | 0x9F,
            0xFF1D => 0xFF,
            0xFF1E => self.nr34 | 0xBF,
            0xFF20 => self.nr41 | 0xFF,
            0xFF21 => self.nr42,
            0xFF22 => self.nr43,
            0xFF23 => self.nr44 | 0xBF,
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                let bit0 = if self.ch1_enabled { 1 } else { 0 };
                let bit1 = if self.ch2_enabled { 2 } else { 0 };
                let bit2 = if self.ch3_enabled { 4 } else { 0 };
                let bit3 = if self.ch4_enabled { 8 } else { 0 };
                let bit7 = if self.nr52 & 0x80 != 0 { 0x80 } else { 0 };
                bit7 | bit3 | bit2 | bit1 | bit0 | 0x70
            }
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize],
            _ => 0xFF,
        }
    }
    pub fn write(&mut self, addr: u16, val: u8) {
        if self.nr52 & 0x80 == 0 && addr != 0xFF26 && !(0xFF30..=0xFF3F).contains(&addr) {
            return; // APU apagada
        }
        match addr {
            0xFF10 => self.nr10 = val,
            0xFF11 => {
                self.nr11 = val;
                self.ch1_length_timer = 64 - (val & 0x3F);
            }
            0xFF12 => self.nr12 = val,
            0xFF13 => self.nr13 = val,
            0xFF14 => {
                self.nr14 = val;
                if val & 0x80 != 0 {
                    // trigger
                    self.ch1_enabled = true;
                    if self.ch1_length_timer == 0 {
                        self.ch1_length_timer = 64;
                    }
                    self.ch1_env_vol = self.nr12 >> 4;
                    self.ch1_env_timer = self.nr12 & 0x07;
                    self.ch1_shadow_freq = (self.nr13 as u16) | (((self.nr14 & 0x07) as u16) << 8);
                    let sweep_pace = (self.nr10 >> 4) & 0x07;
                    self.ch1_sweep_timer = if sweep_pace == 0 { 8 } else { sweep_pace };
                }
            }
            0xFF16 => {
                self.nr21 = val;
                self.ch2_length_timer = 64 - (val & 0x3F);
            }
            0xFF17 => self.nr22 = val,
            0xFF18 => self.nr23 = val,
            0xFF19 => {
                self.nr24 = val;
                if val & 0x80 != 0 {
                    self.ch2_enabled = true;
                    if self.ch2_length_timer == 0 {
                        self.ch2_length_timer = 64;
                    }
                    self.ch2_env_vol = self.nr22 >> 4;
                    self.ch2_env_timer = self.nr22 & 0x07;
                }
            }
            0xFF1A => self.nr30 = val,
            0xFF1B => {
                self.nr31 = val;
                self.ch3_length_timer = 256 - (val as u16);
            }
            0xFF1C => self.nr32 = val,
            0xFF1D => self.nr33 = val,
            0xFF1E => {
                self.nr34 = val;
                if val & 0x80 != 0 {
                    self.ch3_enabled = true;
                    if self.ch3_length_timer == 0 {
                        self.ch3_length_timer = 256;
                    }
                    self.ch3_sample_pos = 0;
                }
            }
            0xFF20 => {
                self.nr41 = val;
                self.ch4_length_timer = 64 - (val & 0x3F);
            }
            0xFF21 => self.nr42 = val,
            0xFF22 => self.nr43 = val,
            0xFF23 => {
                self.nr44 = val;
                if val & 0x80 != 0 {
                    self.ch4_enabled = true;
                    if self.ch4_length_timer == 0 {
                        self.ch4_length_timer = 64;
                    }
                    self.ch4_lfsr = 0x7FFF;
                    self.ch4_env_vol = self.nr42 >> 4;
                    self.ch4_env_timer = self.nr42 & 0x07;
                }
            }
            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,
            0xFF26 => {
                let prev = self.nr52 & 0x80;
                self.nr52 = (self.nr52 & 0x7F) | (val & 0x80);
                if prev != 0 && (val & 0x80) == 0 {
                    // resetear todos los registros al apagar
                    self.ch1_enabled = false;
                    self.ch2_enabled = false;
                    self.ch3_enabled = false;
                    self.ch4_enabled = false;
                }
            }
            0xFF30..=0xFF3F => {
                self.wave_ram[(addr - 0xFF30) as usize] = val;
            }
            _ => {}
        }
    }
}
