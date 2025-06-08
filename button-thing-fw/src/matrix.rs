use cortex_m::delay::Delay;
use debounced_pin::{ActiveHigh, Debounce, DebouncedInputPin};
use embedded_hal::digital::OutputPin;
use keypad::embedded_hal::digital::v2::InputPin;
use midi_convert::midi_types::{Channel, MidiMessage, Note, Value7};
use rp2040_hal::gpio::{DynPinId, FunctionSio, Pin, PullDown, PullUp, SioInput, SioOutput};

pub struct Matrix<'delay> {
    row_pins: [DebouncedInputPin<Pin<DynPinId, FunctionSio<SioInput>, PullUp>, ActiveHigh>; 8],
    col_pins: [Pin<DynPinId, FunctionSio<SioOutput>, PullDown>; 8],
    previous_states: [(Note, bool); 64],
    delay: &'delay mut Delay,
}

impl<'delay> Matrix<'delay> {
    pub fn new(
        row_pins: [Pin<DynPinId, FunctionSio<SioInput>, PullUp>; 8],
        col_pins: [Pin<DynPinId, FunctionSio<SioOutput>, PullDown>; 8],
        delay: &'delay mut Delay,
    ) -> Self {
        let c1_u8 = 3 * 12 + 0;
        let mut previous_states = [(Note::from(c1_u8), false); 64];
        for (i, note) in previous_states.iter_mut().enumerate() {
            let new_note: u8 = c1_u8 + i as u8;
            *note = (Note::from(new_note), false);
        }

        let mut row_pins = row_pins.map(|p| DebouncedInputPin::new(p, ActiveHigh));
        row_pins.reverse();

        Self {
            row_pins,
            col_pins,
            previous_states,
            delay,
        }
    }

    fn update(&mut self) -> anyhow::Result<()> {
        for row_pin in self.row_pins.iter_mut() {
            row_pin.update()?;
        }
        Ok(())
    }

    fn state_col(&mut self, col: usize) -> anyhow::Result<[(Note, bool); 8]> {
        let col_pin = &mut self.col_pins[col];
        col_pin
            .set_high()
            .map_err(|_| anyhow::anyhow!("Failed to set col pin high"))?;
        self.delay.delay_us(20);

        let mut state = [(Note::C1, false); 8];
        for (i, row_pin) in self.row_pins.iter_mut().enumerate() {
            let is_high = row_pin.is_high()?;
            state[i] = (self.previous_states[i + col * 8].0, is_high);
        }

        col_pin
            .set_low()
            .map_err(|_| anyhow::anyhow!("Failed to set col pin low"))?;

        self.delay.delay_us(20);

        Ok(state)
    }

    fn state(&mut self) -> anyhow::Result<[(Note, bool); 64]> {
        let mut state = [(Note::C1, false); 64];
        for col in 0..8 {
            let col_state = self.state_col(col)?;
            state[col * 8..(col + 1) * 8].copy_from_slice(&col_state);
        }
        Ok(state)
    }

    fn diff_state(
        &mut self,
        new_state: [(Note, bool); 64],
    ) -> anyhow::Result<[Option<(Note, bool)>; 64]> {
        let states = self.state()?;
        let mut diff = [None; 64];
        for (i, (new_note, new_pressed)) in new_state.iter().enumerate() {
            let old_note = states[i].0;
            let old_pressed = states[i].1;
            if *new_note != old_note || *new_pressed != old_pressed {
                diff[i] = Some((*new_note, *new_pressed));
            }
        }
        Ok(diff)
    }

    pub fn render_notes(&mut self) -> anyhow::Result<[Option<MidiMessage>; 64]> {
        self.update()?;
        let state = self.state()?;
        let diff = self.diff_state(state)?;
        let mut messages = [None; 64];
        for (i, (note, pressed)) in diff.iter().flatten().enumerate() {
            if *pressed {
                messages[i] = Some(MidiMessage::NoteOn(
                    Channel::from(1),
                    *note,
                    Value7::from(127),
                ));
            } else {
                messages[i] = Some(MidiMessage::NoteOff(
                    Channel::from(1),
                    *note,
                    Value7::from(0),
                ));
            }
        }

        Ok(messages)
    }
}
