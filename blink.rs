#![no_std]
#![no_main]
use tinylua_hal as hal;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }

#[no_mangle]
pub extern "C" fn main() -> ! {
    let mut LED_PIN: i32 = 13;
    hal::pin_mode(LED_PIN  as u8, hal::PinMode::Output);
    loop {
        hal::digital_write(LED_PIN  as u8, true);
        hal::delay_ms(1000 as u32);
        hal::digital_write(LED_PIN  as u8, false);
        hal::delay_ms(1000 as u32);
    }
}
