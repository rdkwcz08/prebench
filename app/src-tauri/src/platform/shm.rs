// app/src-tauri/src/platform/shm.rs — Mémoire partagée du canal flux
// (docs/protocol.md, section 6 ; règle 16.2.6).
//
// Le segment est créé par libvmcu (vmcu_platform_*.c) ; le cœur l'ouvre à la
// réception de STREAM_OPEN. macOS et Linux : shm_open + mmap ; Windows :
// OpenFileMapping + MapViewOfFile. Écrit ici avec libc et windows-sys, déjà
// présentes, plutôt qu'avec une crate de plus.
//
// Le segment vient d'un autre processus : sa taille est vérifiée avant toute
// lecture, et chaque accès est borné. Les positions et numéros de commit de
// l'en-tête sont lus et écrits en atomique (acquire / release), comme libvmcu.
// Étape 6 : positions du tampon circulaire (flux bulk) ; les numéros de commit
// des framebuffers viendront avec les écrans qui les consomment.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

/// Segment projeté en lecture et écriture.
pub struct SharedMemory {
    ptr: *mut u8,
    len: usize,
    #[cfg(windows)]
    mapping: windows_sys::Win32::Foundation::HANDLE,
}

// Le segment n'appartient qu'à cette valeur ; les accès concurrents avec le
// firmware passent par les atomiques de l'en-tête (un producteur, un consommateur).
unsafe impl Send for SharedMemory {}

impl SharedMemory {
    /// Ouvre le segment `name` et en projette `len` octets ; erreur s'il est plus petit.
    pub fn open(name: &str, len: usize) -> io::Result<Self> {
        if len == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "segment vide"));
        }
        imp::open(name, len)
    }

    fn check(&self, offset: usize, size: usize) -> io::Result<()> {
        match offset.checked_add(size) {
            Some(end) if end <= self.len => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "accès hors du segment",
            )),
        }
    }

    /// Copie `out.len()` octets depuis `offset`.
    pub fn read(&self, offset: usize, out: &mut [u8]) -> io::Result<()> {
        self.check(offset, out.len())?;
        // SAFETY : intervalle vérifié dans la projection, qui vit autant que self.
        unsafe { std::ptr::copy_nonoverlapping(self.ptr.add(offset), out.as_mut_ptr(), out.len()) };
        Ok(())
    }

    fn atomic_u64(&self, offset: usize) -> io::Result<&AtomicU64> {
        self.check(offset, 8)?;
        if !offset.is_multiple_of(8) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "champ non aligné",
            ));
        }
        // SAFETY : intervalle vérifié, aligné (projection alignée sur une page) ;
        // l'autre processus n'y accède qu'en atomique.
        Ok(unsafe { &*(self.ptr.add(offset) as *const AtomicU64) })
    }

    pub fn load_u64(&self, offset: usize) -> io::Result<u64> {
        Ok(self.atomic_u64(offset)?.load(Ordering::Acquire))
    }

    pub fn store_u64(&self, offset: usize, value: u64) -> io::Result<()> {
        self.atomic_u64(offset)?.store(value, Ordering::Release);
        Ok(())
    }
}

#[cfg(unix)]
mod imp {
    use super::SharedMemory;
    use std::ffi::CString;
    use std::io;

    pub fn open(name: &str, len: usize) -> io::Result<SharedMemory> {
        let c_name = CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nom de segment invalide"))?;
        // SAFETY : appels libc avec un nom terminé par zéro ; le descripteur est
        // fermé sur tous les chemins, la projection est rendue par Drop.
        unsafe {
            let fd = libc::shm_open(c_name.as_ptr(), libc::O_RDWR, 0);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let mut st: libc::stat = std::mem::zeroed();
            if libc::fstat(fd, &mut st) != 0 {
                let e = io::Error::last_os_error();
                libc::close(fd);
                return Err(e);
            }
            if (st.st_size as u64) < len as u64 {
                libc::close(fd);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "segment plus petit qu'annoncé",
                ));
            }
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            );
            libc::close(fd);
            if ptr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            Ok(SharedMemory {
                ptr: ptr.cast(),
                len,
            })
        }
    }

    impl Drop for SharedMemory {
        fn drop(&mut self) {
            // SAFETY : projection obtenue par mmap avec cette longueur.
            unsafe {
                libc::munmap(self.ptr.cast(), self.len);
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::SharedMemory;
    use std::ffi::CString;
    use std::io;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingA, UnmapViewOfFile, VirtualQuery, FILE_MAP_ALL_ACCESS,
        MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
    };

    pub fn open(name: &str, len: usize) -> io::Result<SharedMemory> {
        let c_name = CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nom de segment invalide"))?;
        // SAFETY : appels Win32 avec un nom terminé par zéro ; poignée et vue
        // rendues sur tous les chemins d'erreur, puis par Drop.
        unsafe {
            let mapping = OpenFileMappingA(FILE_MAP_ALL_ACCESS, 0, c_name.as_ptr().cast());
            if mapping.is_null() {
                return Err(io::Error::last_os_error());
            }
            let view = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, len);
            if view.Value.is_null() {
                let e = io::Error::last_os_error();
                CloseHandle(mapping);
                return Err(e);
            }
            // Taille réelle de la vue : refusée si plus petite qu'annoncé.
            let mut info: MEMORY_BASIC_INFORMATION = std::mem::zeroed();
            let got = VirtualQuery(
                view.Value,
                &mut info,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            );
            if got == 0 || info.RegionSize < len {
                UnmapViewOfFile(view);
                CloseHandle(mapping);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "segment plus petit qu'annoncé",
                ));
            }
            Ok(SharedMemory {
                ptr: view.Value.cast(),
                len,
                mapping,
            })
        }
    }

    impl Drop for SharedMemory {
        fn drop(&mut self) {
            // SAFETY : vue et poignée obtenues dans open.
            unsafe {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr.cast(),
                });
                CloseHandle(self.mapping);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_segment_is_an_error() {
        #[cfg(unix)]
        let name = "/pb_absent_0_0";
        #[cfg(windows)]
        let name = "Local\\prebench_absent_0_0";
        assert!(SharedMemory::open(name, 128).is_err());
        assert!(SharedMemory::open(name, 0).is_err());
    }
}
