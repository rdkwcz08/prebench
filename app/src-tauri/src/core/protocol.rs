// app/src-tauri/src/core/protocol.rs — Trames du protocole v1 (docs/protocol.md).
//
// Miroir de libvmcu/core/vmcu_wire.c : en-tête fixe de 24 octets,
// little-endian, lecture octet par octet. Aucun JSON (règle 16.2.4).

use crate::error::{CoreError, CoreResult};

pub const MAGIC: u16 = 0x4250;
pub const HEADER_SIZE: usize = 24;
pub const MAX_PAYLOAD: usize = 65536;
pub const PROTOCOL_MAJOR: u8 = 1;
pub const PROTOCOL_MINOR: u8 = 1;

#[allow(dead_code)] // posé par le firmware ; le cœur n'émet pas encore de requête SYNC
pub const FLAG_SYNC: u16 = 0x0001;
pub const FLAG_REPLY: u16 = 0x0002;

/// Types de messages (section 4).
pub mod msg {
    pub const HELLO: u16 = 0x0001;
    pub const GPIO_CONFIG: u16 = 0x0010;
    pub const GPIO_WRITE: u16 = 0x0011;
    pub const WARN: u16 = 0x0080;
    pub const LOG: u16 = 0x0081;
    /// Protocole 1.1 : firmware lancé par le débogueur (PREBENCH_DEBUG=1).
    pub const HEARTBEAT: u16 = 0x0090;
    pub const PING: u16 = 0x00F0;

    pub const WELCOME: u16 = 0x8001;
    pub const REJECT: u16 = 0x8002;
    pub const GPIO_INPUT: u16 = 0x8010;
    pub const RESET: u16 = 0x80E0;
    pub const PAUSE: u16 = 0x80E1;
    pub const RESUME: u16 = 0x80E2;
    // Arrêt propre : pas encore utilisé (l'arrêt ferme le socket).
    #[allow(dead_code)]
    pub const SHUTDOWN: u16 = 0x80E3;
    pub const PONG: u16 = 0x80F0;
}

/// Raisons de REJECT (section 4.2).
pub mod reject {
    pub const VERSION: u16 = 1;
    pub const UNKNOWN_BOARD: u16 = 2;
    pub const BAD_BOARD_ID: u16 = 3;
    pub const FRAMEWORK: u16 = 4;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub msg_type: u16,
    pub board_id: u16,
    pub flags: u16,
    pub seq: u32,
    pub length: u32,
    pub timestamp_us: u64,
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut out = [0u8; HEADER_SIZE];
        out[0..2].copy_from_slice(&MAGIC.to_le_bytes());
        out[2..4].copy_from_slice(&self.msg_type.to_le_bytes());
        out[4..6].copy_from_slice(&self.board_id.to_le_bytes());
        out[6..8].copy_from_slice(&self.flags.to_le_bytes());
        out[8..12].copy_from_slice(&self.seq.to_le_bytes());
        out[12..16].copy_from_slice(&self.length.to_le_bytes());
        out[16..24].copy_from_slice(&self.timestamp_us.to_le_bytes());
        out
    }

    pub fn decode(raw: &[u8; HEADER_SIZE]) -> CoreResult<Self> {
        let mut r = Reader::new(raw);
        if r.u16() != MAGIC {
            return Err(CoreError::Protocol("en-tête : magic invalide".into()));
        }
        let h = Header {
            msg_type: r.u16(),
            board_id: r.u16(),
            flags: r.u16(),
            seq: r.u32(),
            length: r.u32(),
            timestamp_us: r.u64(),
        };
        if h.length as usize > MAX_PAYLOAD {
            return Err(CoreError::Protocol(format!(
                "charge utile trop longue : {} octets",
                h.length
            )));
        }
        Ok(h)
    }
}

/// Construit une trame complète (en-tête + charge utile).
pub fn frame(msg_type: u16, board_id: u16, flags: u16, seq: u32, payload: &[u8]) -> Vec<u8> {
    let header = Header {
        msg_type,
        board_id,
        flags,
        seq,
        // La charge utile est bornée par construction (MAX_PAYLOAD).
        length: u32::try_from(payload.len()).unwrap_or(u32::MAX),
        timestamp_us: 0,
    };
    let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
    out.extend_from_slice(&header.encode());
    out.extend_from_slice(payload);
    out
}

/// Lecture séquentielle : au-delà de la fin, renvoie 0 et lève `error`.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    pub error: bool,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader {
            data,
            pos: 0,
            error: false,
        }
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.error || self.data.len() - self.pos < n {
            self.error = true;
            return None;
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }

    pub fn u8(&mut self) -> u8 {
        self.take(1).map_or(0, |s| s[0])
    }

    pub fn u16(&mut self) -> u16 {
        self.take(2).map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
    }

    pub fn u32(&mut self) -> u32 {
        self.take(4)
            .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    pub fn u64(&mut self) -> u64 {
        self.take(8).map_or(0, |s| {
            let mut b = [0u8; 8];
            b.copy_from_slice(s);
            u64::from_le_bytes(b)
        })
    }

    /// str : u16 longueur + UTF-8 (caractères invalides remplacés).
    pub fn str(&mut self) -> String {
        let n = self.u16() as usize;
        self.take(n)
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .unwrap_or_default()
    }
}

/// Écriture d'une charge utile.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }

    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// str tronquée à 1024 octets, sans couper un caractère UTF-8.
    pub fn str(&mut self, s: &str) -> &mut Self {
        let mut n = s.len().min(1024);
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        self.u16(n as u16);
        self.buf.extend_from_slice(&s.as_bytes()[..n]);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Même vecteur que libvmcu/tests/test_wire.c : les deux côtés s'accordent.
    #[test]
    fn header_matches_c_layout() {
        let h = Header {
            msg_type: 0x0011,
            board_id: 0x0102,
            flags: FLAG_SYNC,
            seq: 0x0A0B_0C0D,
            length: 3,
            timestamp_us: 0x1122_3344_5566_7788,
        };
        let expected: [u8; HEADER_SIZE] = [
            b'P', b'B', 0x11, 0x00, 0x02, 0x01, 0x01, 0x00, 0x0D, 0x0C, 0x0B, 0x0A, 0x03, 0x00,
            0x00, 0x00, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11,
        ];
        assert_eq!(h.encode(), expected);
        assert_eq!(Header::decode(&expected).ok(), Some(h));
    }

    #[test]
    fn rejects_bad_magic_and_length() {
        let mut raw = Header {
            msg_type: 1,
            board_id: 1,
            flags: 0,
            seq: 1,
            length: 70000,
            timestamp_us: 0,
        }
        .encode();
        assert!(Header::decode(&raw).is_err());
        raw[0] = b'X';
        assert!(Header::decode(&raw).is_err());
    }

    #[test]
    fn reader_stops_at_end() {
        let mut w = Writer::default();
        w.u16(7).str("nucleo");
        let mut r = Reader::new(&w.buf);
        assert_eq!(r.u16(), 7);
        assert_eq!(r.str(), "nucleo");
        assert!(!r.error);
        assert_eq!(r.u32(), 0);
        assert!(r.error);
    }

    #[test]
    fn writer_truncates_on_char_boundary() {
        let s = format!("{}é", "a".repeat(1023));
        let mut w = Writer::default();
        w.str(&s);
        assert_eq!(u16::from_le_bytes([w.buf[0], w.buf[1]]), 1023);
    }
}
