pub trait Led {
    fn on(&mut self);
    fn off(&mut self);
    fn toggle(&mut self);
}

pub trait Button {
    fn is_pressed(&self) -> bool;
}
