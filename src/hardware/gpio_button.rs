use super::traits::Button;
use embassy_stm32::gpio::Input;

type _CbFun = fn();

pub struct GpioButton<'d> {
    pin: Input<'d>,
}

impl<'d> GpioButton<'d> {
    pub fn new(pin: Input<'d>) -> Self {
        Self { pin }
    }
}

impl<'d> Button for GpioButton<'d> {
    fn is_pressed(&self) -> bool {
        self.pin.is_low()
    }
}
