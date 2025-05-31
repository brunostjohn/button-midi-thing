#![no_std]
#![no_main]

use anyhow::Context;
use board::init_usb_midi;
use bsp::entry;
use bsp::hal::{clocks::init_clocks_and_plls, pac, sio::Sio, watchdog::Watchdog};
use defmt::*;
use defmt_rtt as _;
use globals::{USB_DEVICE, USB_MIDI};
use panic_probe as _;
use rp_pico::hal::pac::interrupt;
use rp_pico::{self as bsp};

mod board;
mod globals;
mod utils;

fn main() -> anyhow::Result<()> {
    info!("Started firmware!");

    let mut pac = pac::Peripherals::take().context("Failed to take peripherals")?;
    let core = pac::CorePeripherals::take().context("Failed to take core peripherals")?;
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .map_err(|e| anyhow::anyhow!("Failed to initialize clocks: {:?}", e))
    .context("Failed to initialize clocks!")?;

    init_usb_midi(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        pac.RESETS,
    )
    .context("Failed to initialize USB MIDI")?;

    Ok(())
}

#[allow(non_snake_case)]
#[interrupt]
unsafe fn USBCTRL_IRQ() {
    let usb_dev = USB_DEVICE.as_mut().unwrap();
    let usb_midi = USB_MIDI.as_mut().unwrap();
    usb_dev.poll(&mut [usb_midi]);
}

#[entry]
fn entry() -> ! {
    if let Err(e) = main() {
        error!("A FATAL ERROR HAS OCCURRED AND EXECUTION CANNOT CONTINUE!");
        // print the damn thing
    }

    loop {}
}
