use bytes::Bytes;
use std::net::SocketAddr;
use std::path::PathBuf;

pub struct SourcePayload {
    pub raw_data: Bytes,
    pub origin: SourceOrigin,
    pub size: usize,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum SourceOrigin {
    File {
        path: PathBuf,
        inode: u64,
        offset: u64,
    },

    Journald {
        unit: String,
        cursor: String,
    },

    Socket {
        peer_addr: SocketAddr,
        protocol: Protocol,
    },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
}

pub struct SourceId {
    pub driver: &'static str,
    pub instance: String,
}
