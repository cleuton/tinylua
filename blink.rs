#![no_std]
#![no_main]
use tinylua_hal as hal;

#[no_mangle]
pub extern "C" fn main() -> ! {
    let mut LED_PIN: i32 = 13;
    hal::pin_mode(LED_PIN, hal::PinMode::Output);
    loop {
        hal::digital_write(LED_PIN, true);
        hal::delay_ms(1000 as u32);
        hal::digital_write(LED_PIN, false);
        hal::delay_ms(1000 as u32);
    }
}
