//! micrphone peripheral
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_05, P0_20, SAADC},
    saadc::*,
};
use embassy_time::{Duration, Timer};

/// Microphone interface
pub struct Microphone<'a> {
    adc: Saadc<'a, 1>,
    enable: Output<'a, P0_20>,
}

impl<'a> Microphone<'a> {
    /// Create a new microphone instance
    pub fn new(saadc: SAADC, irq: interrupt::SAADC, mic: P0_05, micen: P0_20) -> Self {
        let config = Config::default();
        let mut channel_config = ChannelConfig::single_ended(mic);
        channel_config.gain = Gain::GAIN4;
        let saadc = Saadc::new(saadc, irq, config, [channel_config]);
        let enable = Output::new(micen, Level::Low, OutputDrive::HighDrive);
        Self { adc: saadc, enable }
    }

    /// Enable the microphone and return the sound level as detected by the microphone.
    ///
    /// The returned value is a number between 0 and 255 and does not correspond to any official sound level meter number.
    pub async fn sound_level(&mut self) -> u8 {
        self.enable.set_high();
        Timer::after(Duration::from_millis(10)).await;

        let mut bufs = [[[0; 1]; 1024]; 2];

        self.adc
            .run_timer_sampler::<u32, _, 1024>(&mut bufs, 727, move |_| SamplerState::Stopped)
            .await;
        self.enable.set_low();

        let mut max: i16 = i16::MIN;
        let mut min: i16 = i16::MAX;
        for b in bufs[0] {
            if b[0] > max {
                max = b[0];
            }
            if b[0] < min {
                min = b[0];
            }
        }
        let amplitude = max - min;
        // Transpose to u8
        (amplitude / 16) as u8
    }
}
