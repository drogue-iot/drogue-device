use crate::domain::time::duration::Milliseconds;

pub trait Timer {
    fn start(&mut self, duration: Milliseconds);
    fn clear_update_interrupt_flag(&mut self);
}
