use crate::event::Event;
use crate::state::Snapshot;

pub trait Observer: std::fmt::Debug {
    fn on_event(&mut self, event: &Event);
    fn on_snapshot(&mut self, snapshot: &Snapshot);
}
