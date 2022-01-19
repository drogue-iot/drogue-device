pub enum Address {
    Unassigned,
    Unicast(UnicastAddress),
    Virtual(VirtualAddress),
    Group(GroupAddress),
}

pub struct UnicastAddress([u8; 2]);
pub struct VirtualAddress([u8; 2]);
pub struct GroupAddress([u8; 2]);
