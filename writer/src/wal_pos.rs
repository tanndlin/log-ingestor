#[derive(PartialEq, Eq, Clone, Debug)]
pub struct WalPos {
    pub segment: u64,
    pub offset: u64,
}

impl WalPos {
    pub fn new(segment: u64, offset: u64) -> Self {
        Self { segment, offset }
    }
}
