use rp_pico::{
    hal::{self, clocks::UsbClock},
    pac::{RESETS, USBCTRL_DPRAM, USBCTRL_REGS},
};
use usb_device::{
    bus::UsbBusAllocator,
    device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid},
};
use usbd_midi::UsbMidiClass;

use crate::globals::{USB_BUS, USB_DEVICE, USB_MIDI};

pub fn init_usb_midi<'a>(
    ctrl_reg: USBCTRL_REGS,
    ctrl_dparam: USBCTRL_DPRAM,
    usb_clock: UsbClock,
    mut resets: RESETS,
) -> anyhow::Result<()> {
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        ctrl_reg,
        ctrl_dparam,
        usb_clock,
        true,
        &mut resets,
    ));
    unsafe {
        USB_BUS = Some(usb_bus);
    }

    let bus_ref = unsafe { USB_BUS.as_ref() }.ok_or(anyhow::anyhow!("USB bus not initialized"))?;

    let midi = UsbMidiClass::new(bus_ref, 1, 1)
        .map_err(|e| anyhow::anyhow!("Failed to initialize MIDI: {:?}", e))?;
    unsafe {
        USB_MIDI = Some(midi);
    }

    let usb_dev = UsbDeviceBuilder::new(bus_ref, UsbVidPid(0x0420, 0x1337))
        .device_class(0)
        .device_sub_class(0)
        .strings(&[StringDescriptors::default()
            .manufacturer("Zefir's MIDI Bullshit")
            .product("Arcade Button Thing 2000")
            .serial_number("2137")])
        .map_err(|e| anyhow::anyhow!("Failed to build USB device: {:?}", e))?
        .build();

    unsafe {
        USB_DEVICE = Some(usb_dev);
    }

    Ok(())
}
