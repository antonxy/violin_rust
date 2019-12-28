//! Sine wave generator with frequency configuration exposed through standard
//! input.
extern crate crossbeam_channel;
extern crate jack;
extern crate serial;

use crossbeam_channel::bounded;
use std::str::FromStr;

use std::env;
use std::io;
use std::io::BufReader;
use std::time::Duration;

use std::io::prelude::*;
use serial::prelude::*;

struct SineOscillator {
    frequency: f32,
    phase: f32,
    amplitude: f32,
    sampling_rate: usize,
}

impl SineOscillator {
    fn get_frame(&mut self, buffer: &mut [f32]) {
        // generate sample
        for (n, y) in buffer.iter_mut().enumerate() {
            let t: f32 = (n as f32) * (1. / self.sampling_rate as f32);
            let omega: f32 = 2. * std::f32::consts::PI *
(self.frequency);
            *y = (t * omega + self.phase).sin() *
self.amplitude;
        }
        // calculate phase shift
        let phase_shift = (1.
            / ((1. / self.frequency)
                / ((buffer.len() as f32) / (self.sampling_rate as f32))))
            * 2.
            * std::f32::consts::PI;
        self.phase = (self.phase + phase_shift) % (2. *
std::f32::consts::PI);
    }
    fn new(frequency: f32, sampling_rate: usize) -> Self {
        SineOscillator {
            frequency: frequency,
            phase: 0.,
            amplitude: 1.,
            sampling_rate,
        }
    }
}


fn main() {
    let mut port = serial::open("/dev/ttyUSB0").unwrap();
    port.reconfigure(&|settings| {
        settings.set_baud_rate(serial::Baud115200).unwrap();
        settings.set_char_size(serial::Bits8);
        settings.set_parity(serial::ParityNone);
        settings.set_stop_bits(serial::Stop1);
        settings.set_flow_control(serial::FlowNone);
        Ok(())
    }).unwrap();
    port.set_timeout(Duration::from_millis(5000)).unwrap();

    // 1. open a client
    let (client, _status) =
        jack::Client::new("rust_jack_sine", jack::ClientOptions::NO_START_SERVER).unwrap();

    // 2. register port
    let mut out_port = client
        .register_port("sine_out", jack::AudioOut::default())
        .unwrap();

    // 3. define process callback handler
    let sample_rate = client.sample_rate();
    let (tx, rx) = bounded(1_000_000);
    let mut osc = SineOscillator::new(440.0, sample_rate);
    let process = jack::ClosureProcessHandler::new(
        move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
            // Get output buffer
            let out = out_port.as_mut_slice(ps);

            // Check frequency requests
            while let Ok(f) = rx.try_recv() {
                osc.frequency = f;
            }

            osc.get_frame(out);

            // Continue as normal
            jack::Control::Continue
        },
    );

    // 4. activate the client
    let active_client = client.activate_async((), process).unwrap();
    // processing starts here
    let mut reader = BufReader::new(port);

    // 5. wait or do some processing while your handler is running in real time.
    println!("Enter an integer value to change the frequency of the sine wave.");
    while let Some(f) = read_freq(&mut reader) {
        let note = (f - 68.0) / 6.5;
        let scaled = (2 as f32).powf(note/12.0)*440.0;
        tx.send(scaled).unwrap();
        //println!("{} - {}", f, scaled);
    }

    // 6. Optional deactivate. Not required since active_client will deactivate on
    // drop, though explicit deactivate may help you identify errors in
    // deactivate.
    active_client.deactivate().unwrap();
}

fn read_freq(reader: &mut BufReader<serial::SystemPort>) -> Option<f32> {
    let mut user_input = String::new();
    reader.read_line(&mut user_input).ok();
    let v2 = user_input.split(" ").nth(1)?;
    u16::from_str(&v2.trim()).ok().map(|n| n as f32)
}


