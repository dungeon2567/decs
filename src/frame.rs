use crate::tick::Tick;

pub struct Frame {
    pub current_tick: Tick,
}

impl Frame {
    pub fn new(current_tick: Tick) -> Self {
        Self { current_tick }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            current_tick: Tick(0),
        }
    }
}
