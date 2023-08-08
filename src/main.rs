#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

use core::cell::RefCell;

use arduino_hal::{
    delay_us,
    port::{
        mode::{Floating, Input, PullUp},
        Pin,
    },
};
use avr_device::interrupt::{self, Mutex};

use panic_halt as _;

type Console = arduino_hal::hal::usart::Usart0<arduino_hal::DefaultClock>;
static CONSOLE: interrupt::Mutex<RefCell<Option<Console>>> =
    interrupt::Mutex::new(RefCell::new(None));

macro_rules! print {
    ($($t:tt)*) => {
        interrupt::free(
            |cs| {
                if let Some(console) = CONSOLE.borrow(cs).borrow_mut().as_mut() {
                    let _ = ufmt::uwrite!(console, $($t)*);
                }
            },
        )
    };
}

macro_rules! println {
    ($($t:tt)*) => {
        interrupt::free(
            |cs| {
                if let Some(console) = CONSOLE.borrow(cs).borrow_mut().as_mut() {
                    let _ = ufmt::uwriteln!(console, $($t)*);
                }
            },
        )
    };
}

macro_rules! print_bytes {
    ($bytes: expr) => {
        interrupt::free(|cs| {
            if let Some(console) = CONSOLE.borrow(cs).borrow_mut().as_mut() {
                $bytes.iter().for_each(|b| console.write_byte(*b));
            }
        })
    };
}

macro_rules! dbg {
    ($t: expr) => {{
        let t = $t;
        println!("{} = {}", stringify!($t), t);
        t
    }};
}

fn char_from_scancode(scancode: u8, shift: bool) -> char {
    let c = match scancode {
        0x1c => Some('a'),
        0x32 => Some('b'),
        0x21 => Some('c'),
        0x23 => Some('d'),
        0x24 => Some('e'),
        0x2b => Some('f'),
        0x34 => Some('g'),
        0x33 => Some('h'),
        0x43 => Some('i'),
        0x3b => Some('j'),
        0x42 => Some('k'),
        0x4b => Some('l'),
        0x3a => Some('m'),
        0x31 => Some('n'),
        0x44 => Some('o'),
        0x4d => Some('p'),
        0x15 => Some('q'),
        0x2d => Some('r'),
        0x1b => Some('s'),
        0x2c => Some('t'),
        0x3c => Some('u'),
        0x2a => Some('v'),
        0x1d => Some('w'),
        0x22 => Some('x'),
        0x35 => Some('y'),
        0x1a => Some('z'),
        _ => None,
    };

    if let Some(c) = c {
        return (c as u8 - (b'a' - b'A') * shift as u8) as char;
    }

    match scancode {
        0x16 if shift => '!',
        0x16 => '1',
        0x1e if shift => '"',
        0x1e => '2',
        0x26 if shift => '£',
        0x26 => '3',
        0x25 if shift => '$',
        0x25 => '4',
        0x2e if shift => '%',
        0x2e => '5',
        0x36 if shift => '^',
        0x36 => '6',
        0x3d if shift => '&',
        0x3d => '7',
        0x3e if shift => '*',
        0x3e => '8',
        0x46 if shift => '(',
        0x46 => '9',
        0x45 if shift => ')',
        0x45 => '0',
        0x29 => ' ',
        0x41 if shift => '<',
        0x41 => ',',
        0x49 if shift => '>',
        0x49 => '.',
        0x4a if shift => '?',
        0x4a => '/',
        0x4c if shift => ':',
        0x4c => ';',
        0x4e if shift => '_',
        0x4e => '-',
        0x52 if shift => '@',
        0x52 => '\'',
        0x54 if shift => '{',
        0x54 => '[',
        0x5b if shift => '}',
        0x5b => ']',
        0x55 if shift => '+',
        0x55 => '=',
        0x5a => '\n',
        0x5d if shift => '#',
        0x5d => '\\',

        // keypad
        0x69 => '1',
        0x6b => '4',
        0x6c => '7',
        0x70 => '0',
        0x71 => '.',
        0x72 => '2',
        0x73 => '5',
        0x74 => '6',
        0x75 => '8',
        0x79 => '+',
        0x7a => '3',
        0x7b => '-',
        0x7c => '*',
        0x7d => '9',

        0x0d => '\t',

        _ => '?',
    }
}

/// This does not currently work.  I'm not sure why...
/// If I knew, then I'd fix it :P
fn send_byte(b: u8) {
    interrupt::free(|cs| {
        let clock = CLOCK_PIN.borrow(cs).take().unwrap();
        let mut clock = clock.into_output(); // CLK -> Low
        let data = DATA_PIN.borrow(cs).take().unwrap();
        let mut data = data.into_output_high(); // DAT -> High

        delay_us(200); // ..
        data.set_low(); // DAT -> Low
        delay_us(30); // ..
        clock.set_high(); // CLK -> High

        // release the clock signal
        let clock = clock.into_floating_input();

        while clock.is_high() {}
        for bit in 0..=9 {
            if bit == 9 || bit == 10 {
                data.set_high();
            } else if bit == 8 {
                //println!("parity");
                if b.count_ones() & 1 == 0 {
                    data.set_high();
                } else {
                    data.set_low();
                }
            } else {
                //println!("bit {}", bit);
                if b & (1 << bit) != 0 {
                    data.set_high()
                } else {
                    data.set_low()
                }
            }
            while clock.is_low() {}
            while clock.is_high() {}
        }

        println!("release data line");
        let data = data.into_floating_input();

        println!("wait for data low");
        while data.is_high() {}
        println!("wait for clock low");
        while clock.is_high() {}

        println!("okay");

        // Make them input again
        *CLOCK_PIN.borrow(cs).borrow_mut() = Some(clock.into_pull_up_input());
        *DATA_PIN.borrow(cs).borrow_mut() = Some(data.into_floating_input());
    });
}

static CLOCK_PIN: Mutex<RefCell<Option<Pin<Input<PullUp>>>>> = Mutex::new(RefCell::new(None));
static DATA_PIN: Mutex<RefCell<Option<Pin<Input<Floating>>>>> = Mutex::new(RefCell::new(None));

fn put_console(console: Console) {
    interrupt::free(|cs| {
        *CONSOLE.borrow(cs).borrow_mut() = Some(console);
    })
}

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    let serial = arduino_hal::default_serial!(dp, pins, 57600);
    put_console(serial);

    interrupt::free(|cs| {
        *CLOCK_PIN.borrow(cs).borrow_mut() = Some(pins.d2.into_pull_up_input().downgrade());
        *DATA_PIN.borrow(cs).borrow_mut() = Some(pins.d3.into_floating_input().downgrade());
    });

    print!("\n\n\n\n\n---\n");

    let mut pwr = pins.d12.into_output().downgrade();
    pwr.set_high();

    //delay_ms(5000);
    //println!("send");
    //send_byte(0xee);
    //serial.write_str("send data").unwrap();
    //send_byte(0b1000_0000);

    let mut packet = 0u16;
    let mut bits_received = 0;
    let mut code = 0u8;
    let mut parity = false;
    let mut lst_clk = false;

    let mut bitset = 0u8;

    let mut extended = false;
    let mut release = false;

    const L_SHIFT: u8 = 0b0001_0000;
    const L_CTRL: u8 = 0b0010_0000;
    const L_ALT: u8 = 0b0100_0000;
    const L_MOD: u8 = 0b1000_0000;

    const R_SHIFT: u8 = 0b0000_0001;
    const R_CTRL: u8 = 0b0000_0010;
    const R_ALT: u8 = 0b0000_0100;
    const R_MOD: u8 = 0b0000_1000;

    const SHIFT: u8 = L_SHIFT | R_SHIFT;
    const CTRL: u8 = L_CTRL | R_CTRL;
    const ALT: u8 = L_ALT | R_ALT;
    const MOD: u8 = L_MOD | R_MOD;

    loop {
        interrupt::free(|cs| {
            let mut clock = CLOCK_PIN.borrow(cs).borrow_mut();
            let clock = clock.as_mut().unwrap();
            let mut data = DATA_PIN.borrow(cs).borrow_mut();
            let data = data.as_mut().unwrap();

            let new_clk = clock.is_high();

            if !new_clk && lst_clk {
                let dat = u8::from(data.is_high());
                packet <<= 1;
                packet |= dat as u16;

                // bits for scan code (LSB .. MSB)
                if bits_received >= 1 && bits_received <= 8 {
                    code |= dat << (bits_received - 1);
                }

                // parity bit
                if bits_received == 9 {
                    parity = data.is_high();
                }
                bits_received += 1;

                if bits_received == 11 {
                    if code.count_ones() & 1 == u32::from(parity) {
                        println!("\nPARITY FAIL");
                    }

                    macro_rules! handle_bitset {
                        ($mask: ident) => {
                            if release {
                                bitset &= !$mask;
                            } else {
                                bitset |= $mask;
                            }
                        };
                    }

                    let mut reset_extend_release = true;
                    match code {
                        0xe0 => {
                            extended = true;
                            reset_extend_release = false;
                        }
                        0xf0 => {
                            release = true;
                            reset_extend_release = false;
                        }

                        // L Shift
                        0x12 => handle_bitset!(L_SHIFT),
                        // R Shift
                        0x59 => handle_bitset!(R_SHIFT),
                        // R Ctrl
                        0x14 if extended => handle_bitset!(R_CTRL),
                        // L Ctrl
                        0x14 => handle_bitset!(L_CTRL),
                        // R Alt
                        0x11 if extended => handle_bitset!(R_ALT),
                        // L Alt
                        0x11 => handle_bitset!(L_ALT),
                        // R Mod
                        0x27 => handle_bitset!(R_MOD),
                        // L Mod
                        0x1f => handle_bitset!(L_MOD),

                        // Escape
                        0x76 => {
                            print!("[ESC]");
                        }

                        // Backspace
                        0x66 if !release => {
                            //            BKSP  CLEAR BKSP
                            print_bytes!([0x08, b' ', 0x08]);
                        }

                        // Keyboard self-test pass
                        0xaa => {
                            println!("Self-test passed");
                        }

                        _ if !release && !extended => {
                            let c = char_from_scancode(code, (bitset & SHIFT) != 0);
                            if bitset & CTRL != 0 {
                                print!("<C-{}>", c);
                            } else if bitset & ALT != 0 {
                                print!("<M-{}>", c);
                            } else if bitset & MOD != 0 {
                                print!("<S-{}>", c);
                            } else {
                                print!("{}", c)
                            }
                        }

                        _ => {}
                    }

                    if reset_extend_release {
                        extended = false;
                        release = false;
                    }
                    packet = 0;
                    parity = false;
                    bits_received = 0;
                    code = 0;
                }
            }
            lst_clk = new_clk;
        });
    }
}
