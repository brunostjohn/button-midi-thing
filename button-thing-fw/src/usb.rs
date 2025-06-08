use crate::{
    globals::{USB_DEVICE, USB_MIDI},
    SYSEX_BUFFER_SIZE,
};
use anyhow::anyhow;
use defmt::info;
use heapless::Vec;
use midi_convert::{
    midi_types::MidiMessage, parse::MidiTryParseSlice, render_slice::MidiRenderSlice,
};
use usb_device::UsbError;
use usbd_midi::{CableNumber, UsbMidiEventPacket, UsbMidiPacketReader};

pub fn send_midi_message(message: MidiMessage) -> anyhow::Result<usize> {
    let usb_midi = unsafe { USB_MIDI.as_mut() }.ok_or(anyhow!("USB MIDI not initialized"))?;

    let mut buf = [0; 3];
    message.render_slice(&mut buf);
    usb_midi
        .send_packet(
            UsbMidiEventPacket::try_from_payload_bytes(CableNumber::Cable0, &buf)
                .map_err(|e| anyhow!("Failed to render MIDI message: {:?}", e))?,
        )
        .map_err(|e| anyhow!("Failed to send MIDI packet: {:?}", e))
}

// this is safe because we're running on a single thread and in a critical section
pub fn poll_usb_midi() -> anyhow::Result<bool> {
    critical_section::with(|_| {
        let usb_device =
            unsafe { USB_DEVICE.as_mut() }.ok_or(anyhow!("USB device not initialized"))?;
        let usb_midi = unsafe { USB_MIDI.as_mut() }.ok_or(anyhow!("USB MIDI not initialized"))?;

        Ok(usb_device.poll(&mut [usb_midi]))
    })
}

pub fn handle_midi_packets(
    mut handle_message: impl FnMut(MidiMessage) -> anyhow::Result<()>,
    sysex_buffer: &mut Vec<u8, SYSEX_BUFFER_SIZE>,
) -> anyhow::Result<()> {
    let usb_midi = unsafe { USB_MIDI.as_mut() }.ok_or(anyhow!("USB MIDI not initialized"))?;

    let mut buffer = [0; 64];

    let size = usb_midi
        .read(&mut buffer)
        .map_err(|e| anyhow!("Failed to read MIDI packet: {:?}", e))?;
    let packet_reader = UsbMidiPacketReader::new(&buffer, size);

    for packet in packet_reader.into_iter().flatten() {
        if packet.is_sysex() {
            handle_sysex_packets(&packet, sysex_buffer)?;
        } else {
            let message = MidiMessage::try_parse_slice(packet.payload_bytes())
                .map_err(|_| anyhow!("failed to parse MIDI message"))?;
            handle_message(message)?;
        }
    }

    Ok(())
}

fn handle_sysex_packets(
    packet: &UsbMidiEventPacket,
    sysex_buffer: &mut Vec<u8, SYSEX_BUFFER_SIZE>,
) -> anyhow::Result<()> {
    if packet.is_sysex_start() {
        info!("sysex start");
        sysex_buffer.clear();
    }

    sysex_buffer
        .extend_from_slice(packet.payload_bytes())
        .map_err(|_| anyhow!("sysex buffer full!"))?;

    if !packet.is_sysex_end() {
        return Ok(());
    }

    info!("sysex end");
    let response = process_sysex(sysex_buffer.as_ref()).ok_or(anyhow!("no sysex response"))?;
    let usb_midi = unsafe { USB_MIDI.as_mut() }.ok_or(anyhow!("USB MIDI not initialized"))?;
    for good_packet in response
        .chunks(3)
        .flat_map(|c| UsbMidiEventPacket::try_from_payload_bytes(CableNumber::Cable0, c))
    {
        let result = usb_midi.send_packet(good_packet);

        match result {
            Ok(_) => break,
            Err(UsbError::WouldBlock) => continue,
            Err(e) => return Err(anyhow!("Failed to send MIDI packet: {:?}", e)),
        }
    }

    Ok(())
}

pub fn process_sysex(request: &[u8]) -> Option<Vec<u8, SYSEX_BUFFER_SIZE>> {
    /// Identity request message.
    ///
    /// See section *DEVICE INQUIRY* of the *MIDI 1.0 Detailed Specification* for further details.
    const IDENTITY_REQUEST: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7];

    if request == IDENTITY_REQUEST {
        let mut response = Vec::<u8, SYSEX_BUFFER_SIZE>::new();
        response
            .extend_from_slice(&[
                0xF0, 0x7E, 0x7F, 0x06, 0x02, // Header
                0x01, // Manufacturer ID
                0x02, // Family code
                0x03, // Family code
                0x04, // Family member code
                0x05, // Family member code
                0x00, // Software revision level
                0x00, // Software revision level
                0x00, // Software revision level
                0x00, // Software revision level
                0xF7,
            ])
            .ok();

        return Some(response);
    }

    None
}
