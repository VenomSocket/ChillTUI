use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentResult {
    pub title: String,
    #[serde(rename = "source")]
    pub indexer: String,
    pub size: u64,
    pub seeders: u32,
    #[serde(rename = "peers")]
    pub leechers: u32,
    #[serde(rename = "link")]
    pub magnet: String,
    #[serde(skip)]
    pub selected: bool,
}

impl TorrentResult {
    pub fn size_str(&self) -> String {
        const KIB: f64 = 1024.0;
        const MIB: f64 = 1024.0 * 1024.0;
        const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

        let size = self.size as f64;
        if size >= GIB {
            format!("{:.2} GiB", size / GIB)
        } else if size >= MIB {
            format!("{:.2} MiB", size / MIB)
        } else if size >= KIB {
            format!("{:.2} KiB", size / KIB)
        } else {
            format!("{} B", size)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PutioFile {
    pub id: u64,
    pub name: String,
    pub parent_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct PutioTransferResponse {
    pub transfer: PutioTransfer,
}

#[derive(Debug, Deserialize)]
pub struct PutioTransfer {
    pub id: u64,
    pub name: String,
}