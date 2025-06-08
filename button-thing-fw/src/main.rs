#![no_std]
#![no_main]

use anyhow::Context;
use board::init_usb_midi;
use cortex_m::delay::Delay;

use defmt::*;
use defmt_rtt as _;
use embedded_alloc::LlffHeap as Heap;

use globals::USB_MIDI;
use heapless::Vec;

use midi_convert::midi_types::MidiMessage;
use midi_convert::parse::MidiTryParseSlice;

use panic_probe as _;

use rp2040_hal::clocks::init_clocks_and_plls;
use rp2040_hal::gpio::{PinState, Pins};
use rp2040_hal::pac::interrupt;
use rp2040_hal::pio::PIOExt;
use rp2040_hal::{entry, Clock, Sio, Timer, Watchdog};
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use usb_device::UsbError;
use usbd_midi::{CableNumber, UsbMidiEventPacket, UsbMidiPacketReader};
use ws2812_pio::Ws2812;

use crate::matrix::Matrix;
use crate::usb::{handle_midi_packets, poll_usb_midi, send_midi_message};

mod board;
mod colour;
mod globals;
mod matrix;
mod usb;

const SYSEX_BUFFER_SIZE: usize = 64;

fn main() -> anyhow::Result<()> {
    info!("Started firmware!");

    let mut pac = rp2040_hal::pac::Peripherals::take().context("Failed to take peripherals")?;
    let core =
        rp2040_hal::pac::CorePeripherals::take().context("Failed to take core peripherals")?;
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
    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    init_usb_midi(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        &mut pac.RESETS,
    )
    .context("Failed to initialize USB MIDI")?;

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);

    // let mut ws = Ws2812::new(
    //     pins.gpio15.into_function(),
    //     &mut pio,
    //     sm0,
    //     clocks.peripheral_clock.freq(),
    //     timer.count_down(),
    // );

    let mut delay = Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let mut sysex_receive_buffer = Vec::<u8, SYSEX_BUFFER_SIZE>::new();

    let mut matrix = Matrix::new(
        [
            pins.gpio3.into_pull_up_input().into_dyn_pin(),
            pins.gpio5.into_pull_up_input().into_dyn_pin(),
            pins.gpio11.into_pull_up_input().into_dyn_pin(),
            pins.gpio6.into_pull_up_input().into_dyn_pin(),
            pins.gpio7.into_pull_up_input().into_dyn_pin(),
            pins.gpio8.into_pull_up_input().into_dyn_pin(),
            pins.gpio9.into_pull_up_input().into_dyn_pin(),
            pins.gpio10.into_pull_up_input().into_dyn_pin(),
        ],
        [
            pins.gpio22
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
            pins.gpio23
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
            pins.gpio24
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
            pins.gpio25
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
            pins.gpio29
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
            pins.gpio28
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
            pins.gpio27
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
            pins.gpio26
                .into_push_pull_output_in_state(PinState::Low)
                .into_dyn_pin(),
        ],
        &mut delay,
    );

    loop {
        print_if_err(|| {
            handle_midi(&mut sysex_receive_buffer)?;
            emit_changed_notes(&mut matrix)?;

            Ok(())
        });
    }
}

fn print_if_err<F>(mut f: F)
where
    F: FnMut() -> anyhow::Result<()>,
{
    let res = f();
    if let Err(e) = res {
        error!("Failed to run function");
    }
}

fn emit_changed_notes(matrix: &mut Matrix<'_>) -> anyhow::Result<()> {
    let notes = matrix.render_notes()?;

    for changed_note in notes.as_ref().iter().flatten() {
        if let Err(e) = send_midi_message(*changed_note) {
            // find way to print
            error!("Failed to send MIDI message!");
        }
    }

    Ok(())
}

fn handle_midi(sysex_receive_buffer: &mut Vec<u8, SYSEX_BUFFER_SIZE>) -> anyhow::Result<()> {
    if !poll_usb_midi()? {
        return Ok(());
    }

    handle_midi_packets(
        |message| match message {
            _ => Ok(()),
        },
        sysex_receive_buffer,
    )
}

#[allow(non_snake_case)]
#[interrupt]
unsafe fn USBCTRL_IRQ() {
    critical_section::with(|_| {
        poll_usb_midi().expect("USB and MIDI should be initialized");
    });
}

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[entry]
fn entry() -> ! {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
    }
    info!("Heap initialized");

    if let Err(e) = main() {
        error!("A FATAL ERROR HAS OCCURRED AND EXECUTION CANNOT CONTINUE!");
        // print the damn thing
    }

    loop {}
}
