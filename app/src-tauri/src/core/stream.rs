// app/src-tauri/src/core/stream.rs — Flux en mémoire partagée, côté cœur
// (docs/protocol.md, section 6 ; phase 3, étape 6).
//
// libvmcu crée le segment et l'annonce par STREAM_OPEN ; le cœur l'ouvre
// (platform/shm.rs), vérifie l'en-tête et le garde jusqu'à STREAM_CLOSE.
// Étape 6 : flux « bulk » (écritures SPI de plus de 4096 octets), lu par le
// cœur en traitant chaque SPI_XFER, qui en donne la position. Les écrans à
// framebuffer (kind 1) et l'audio viendront avec leurs consommateurs.
//
// Tout ce qui vient du segment est vérifié : magic, version, type, capacité
// annoncée dans la trame, positions (aucune lecture au-delà de ce que le
// producteur a publié, ni d'octets déjà rendus).

use crate::platform::shm::SharedMemory;

pub const HEADER_SIZE: usize = 64;
const MAGIC: u32 = 0x4D53_4250;
const LAYOUT_V1: u16 = 1;
/// Capacité maximale d'un flux (section 6.1).
pub const MAX_CAPACITY: u32 = 16 * 1024 * 1024;
/// Flux ouverts en même temps par un firmware (borne, règle 16.2.11).
pub const MAX_STREAMS: usize = 16;

pub const KIND_BULK: u8 = 5;

const OFF_WRITE_POS: usize = 24;
const OFF_READ_POS: usize = 32;

/// Statuts de STREAM_OPENED (codes vmcu_status_t).
pub mod status {
    pub const OK: i16 = 0;
    pub const INVALID_ARG: i16 = -2;
    pub const UNSUPPORTED: i16 = -3;
    pub const NO_MEMORY: i16 = -7;
    pub const BUSY: i16 = -11;
}

pub struct Stream {
    mem: SharedMemory,
    capacity: u32,
}

impl Stream {
    /// Ouvre et vérifie un segment annoncé par STREAM_OPEN. Err : statut à renvoyer.
    pub fn open(name: &str, kind: u8, capacity: u32) -> Result<Stream, i16> {
        if !(1..=5).contains(&kind) || capacity == 0 || capacity > MAX_CAPACITY {
            return Err(status::INVALID_ARG);
        }
        if kind != KIND_BULK {
            return Err(status::UNSUPPORTED); // framebuffer, audio, caméra : phases suivantes
        }
        let mem = SharedMemory::open(name, HEADER_SIZE + capacity as usize)
            .map_err(|_| status::NO_MEMORY)?;
        let mut head = [0u8; 12];
        mem.read(0, &mut head).map_err(|_| status::INVALID_ARG)?;
        let magic = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        let layout = u16::from_le_bytes([head[4], head[5]]);
        let cap = u32::from_le_bytes([head[8], head[9], head[10], head[11]]);
        if magic != MAGIC || layout != LAYOUT_V1 || head[6] != kind || cap != capacity {
            return Err(status::INVALID_ARG);
        }
        Ok(Stream { mem, capacity })
    }

    /// Octets [position, position + len) du tampon circulaire, puis read_pos
    /// avancé : la place est rendue au firmware.
    pub fn take(&self, position: u64, len: usize) -> Result<Vec<u8>, String> {
        let cap = self.capacity as u64;
        let end = position
            .checked_add(len as u64)
            .ok_or_else(|| "position du flux invalide".to_string())?;
        let write_pos = self
            .mem
            .load_u64(OFF_WRITE_POS)
            .map_err(|e| e.to_string())?;
        let read_pos = self.mem.load_u64(OFF_READ_POS).map_err(|e| e.to_string())?;
        if len as u64 > cap || position < read_pos || end > write_pos {
            return Err(format!(
                "flux bulk : octets {position}..{end} hors de la zone publiée {read_pos}..{write_pos}"
            ));
        }
        let mut out = vec![0u8; len];
        let at = (position % cap) as usize;
        let first = len.min(self.capacity as usize - at);
        self.mem
            .read(HEADER_SIZE + at, &mut out[..first])
            .map_err(|e| e.to_string())?;
        self.mem
            .read(HEADER_SIZE, &mut out[first..])
            .map_err(|e| e.to_string())?;
        self.mem
            .store_u64(OFF_READ_POS, end)
            .map_err(|e| e.to_string())?;
        Ok(out)
    }
}
