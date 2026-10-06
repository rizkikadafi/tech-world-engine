#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub id: ResourceId,
    pub name: String,
    pub capacity: u64,
    pub available: u64,
}

impl Resource {
    pub fn new<S: Into<String>>(id: ResourceId, name: S, capacity: u64) -> Self {
        Self {
            id,
            name: name.into(),
            capacity,
            available: capacity,
        }
    }

    pub fn allocate(&mut self, amount: u64) -> bool {
        if self.available >= amount {
            self.available -= amount;
            true
        } else {
            false
        }
    }

    pub fn release(&mut self, amount: u64) {
        self.available = (self.available + amount).min(self.capacity);
    }
}
