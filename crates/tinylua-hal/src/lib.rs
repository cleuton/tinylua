#![no_std]
#![feature(asm_experimental_arch)]

pub enum PinMode {
    Output,
    Input,
}

// ── Registradores ─────────────────────────────────────────────────────────────

const PORTB: *mut u8 = 0x25 as *mut u8;
const DDRB:  *mut u8 = 0x24 as *mut u8;
const PINB:  *mut u8 = 0x23 as *mut u8;

const PORTC: *mut u8 = 0x28 as *mut u8;
const DDRC:  *mut u8 = 0x27 as *mut u8;
const PINC:  *mut u8 = 0x26 as *mut u8;

const PORTD: *mut u8 = 0x2B as *mut u8;
const DDRD:  *mut u8 = 0x2A as *mut u8;
const PIND:  *mut u8 = 0x29 as *mut u8;

// Registradores ADC
const ADMUX:  *mut u8 = 0x7C as *mut u8;
const ADCSRA: *mut u8 = 0x7A as *mut u8;
const ADCL:   *mut u8 = 0x78 as *mut u8;
const ADCH:   *mut u8 = 0x79 as *mut u8;

// ── Mapeamento pino Arduino → (DDR, PORT, PIN, bit) ──────────────────────────

fn pin_regs(pin: u8) -> (*mut u8, *mut u8, *mut u8, u8) {
    match pin {
        0..=7   => (DDRD, PORTD, PIND, pin),
        8..=13  => (DDRB, PORTB, PINB, pin - 8),
        14..=19 => (DDRC, PORTC, PINC, pin - 14),
        _       => (DDRD, PORTD, PIND, 0), // fallback seguro
    }
}

// ── API pública ───────────────────────────────────────────────────────────────

pub fn pin_mode(pin: u8, mode: PinMode) {
    let (ddr, _, _, bit) = pin_regs(pin);
    unsafe {
        match mode {
            PinMode::Output => *ddr |=  1 << bit,
            PinMode::Input  => *ddr &= !(1 << bit),
        }
    }
}

pub fn digital_write(pin: u8, val: bool) {
    let (_, port, _, bit) = pin_regs(pin);
    unsafe {
        if val { *port |=  1 << bit; }
        else   { *port &= !(1 << bit); }
    }
}

pub fn digital_read(pin: u8) -> bool {
    let (_, _, pin_reg, bit) = pin_regs(pin);
    unsafe { (*pin_reg & (1 << bit)) != 0 }
}

pub fn analog_read(pin: u8) -> u16 {
    // pin aqui é 0-5 (A0-A5); converte para canal ADC
    let channel = if pin >= 14 { pin - 14 } else { pin };
    unsafe {
        // Seleciona canal, referência AVcc (0x40)
        *ADMUX = 0x40 | (channel & 0x07);
        // Habilita ADC, inicia conversão, prescaler 128 (0xC7)
        *ADCSRA = 0xC7;
        // Aguarda conversão (bit ADSC, bit 6)
        while (*ADCSRA & (1 << 6)) != 0 {}
        // Lê resultado (ADCL primeiro, depois ADCH)
        let low  = *ADCL as u16;
        let high = *ADCH as u16;
        (high << 8) | low
    }
}

pub fn analog_write(pin: u8, val: u8) {
    // PWM via Timer0 (pino 6) e Timer1 (pinos 9 e 10)
    // Pino 9 → OC1A (OCR1AL: 0x88), Pino 10 → OC1B (OCR1BL: 0x86)
    // Pino 6 → OC0A (OCR0A: 0x47),  Pino 5 → OC0B (OCR0B: 0x48)
    let ocr: *mut u8 = match pin {
        5  => 0x48 as *mut u8, // OCR0B
        6  => 0x47 as *mut u8, // OCR0A
        9  => 0x88 as *mut u8, // OCR1AL
        10 => 0x86 as *mut u8, // OCR1BL
        11 => 0x84 as *mut u8, // OCR2A
        3  => 0x83 as *mut u8, // OCR2B
        _  => return,           // pino sem PWM — ignora
    };
    unsafe { *ocr = val; }
}

pub fn delay_ms(ms: u32) {
    // Busy-wait: ~16 ciclos por iteração a 16MHz ≈ 1µs por iteração
    // 1ms ≈ 1000 iterações
    for _ in 0..ms {
        for _ in 0..1_000u16 {
            unsafe { core::arch::asm!("nop"); }
        }
    }
}
