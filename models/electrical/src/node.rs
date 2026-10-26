/// Unique identifier for an electrical circuit node.
/// Node 0 is reserved as the reference Ground (GND).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

impl NodeId {
    /// Reference Ground node (0 V).
    pub const GROUND: NodeId = NodeId(0);

    /// Checks if this node is the reference Ground.
    #[inline]
    pub fn is_ground(&self) -> bool {
        self.0 == 0
    }

    /// Internal index of the node.
    #[inline]
    pub fn id(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_ground() {
            write!(f, "GND")
        } else {
            write!(f, "Node({})", self.0)
        }
    }
}
