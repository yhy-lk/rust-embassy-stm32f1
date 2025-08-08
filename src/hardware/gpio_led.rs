use super::traits::Led;
use embassy_stm32::gpio::Output;

pub struct GpioLed<'d> {
    pin: Output<'d>,
}

impl<'d> GpioLed<'d> {
    pub fn new(pin: Output<'d>) -> Self {
        Self { pin }
    }
}

impl<'d> Led for GpioLed<'d> {
    fn on(&mut self) {
        self.pin.set_low();
    }

    fn off(&mut self) {
        self.pin.set_high();
    }

    fn toggle(&mut self) {
        self.pin.toggle();
    }
}
