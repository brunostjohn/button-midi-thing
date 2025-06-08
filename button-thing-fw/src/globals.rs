use rp2040_hal::usb;
use usb_device::{bus::UsbBusAllocator, device::UsbDevice};
use usbd_midi::UsbMidiClass;

pub static mut USB_DEVICE: Option<UsbDevice<usb::UsbBus>> = None;
pub static mut USB_BUS: Option<UsbBusAllocator<usb::UsbBus>> = None;
pub static mut USB_MIDI: Option<UsbMidiClass<usb::UsbBus>> = None;
