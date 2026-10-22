use crate::wal_pos::WalPos;

#[derive(Debug)]
pub enum Event {
    Committed(WalPos),
}
