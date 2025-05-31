use rp_pico::hal;
use usb_device::{bus::UsbBusAllocator, device::UsbDevice};
use usbd_midi::UsbMidiClass;

pub static mut USB_DEVICE: Option<UsbDevice<hal::usb::UsbBus>> = None;
pub static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;
pub static mut USB_MIDI: Option<UsbMidiClass<hal::usb::UsbBus>> = None;
