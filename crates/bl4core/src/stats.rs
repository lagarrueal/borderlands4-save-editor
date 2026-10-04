//! Item card statistics (filled in by the stats engine).
use crate::db::Db;
use crate::item::ItemInfo;
use crate::serial::Serial;

pub fn card(_db: &Db, _s: &Serial, _info: &ItemInfo) -> Vec<(String, String)> {
    vec![]
}
