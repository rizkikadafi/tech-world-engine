use crate::event::Event;
use crate::observation::Observer;
use crate::state::Snapshot;

#[derive(Debug, Default, Clone)]
pub struct EventLogObserver {
    events: Vec<Event>,
    snapshots: Vec<Snapshot>,
}

impl EventLogObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }
}

impl Observer for EventLogObserver {
    fn on_event(&mut self, event: &Event) {
        self.events.push(event.clone());
    }

    fn on_snapshot(&mut self, snapshot: &Snapshot) {
        self.snapshots.push(snapshot.clone());
    }
}
