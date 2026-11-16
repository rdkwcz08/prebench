// app/src-tauri/src/core/components/js_bytes.rs — Octets d'un Uint8Array rendu
// par un modèle de composant (réponse I2C, puis pixels de blit à l'étape 3).
//
// Seul endroit du cœur qui lit la mémoire de QuickJS : la lecture brute est
// enfermée dans `copy`, qui vérifie la cohérence des longueurs, le détachement
// du tableau et la borne demandée avant de copier, puis rend des octets à
// l'appelant. Aucun JavaScript ne s'exécute pendant la copie.

use rquickjs::TypedArray;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BytesError {
    #[error("tableau d'octets détaché (ArrayBuffer transféré ou libéré)")]
    Detached,
    #[error("tableau d'octets incohérent : {view} éléments annoncés, {raw} octets lisibles")]
    Inconsistent { view: usize, raw: usize },
}

/// Octets copiés, et longueur totale du tableau (supérieure à `data.len()`
/// quand il dépassait la borne demandée).
#[derive(Debug, PartialEq, Eq)]
pub struct Bytes {
    pub data: Vec<u8>,
    pub total: usize,
}

/// Copie au plus `max` octets d'un Uint8Array, vue partielle d'un
/// ArrayBuffer comprise (le décalage de la vue est appliqué par QuickJS).
pub fn copy(array: &TypedArray<'_, u8>, max: usize) -> Result<Bytes, BytesError> {
    let view = array.len();
    let raw = array.as_raw().ok_or(BytesError::Detached)?;
    if raw.len() != view {
        return Err(BytesError::Inconsistent {
            view,
            raw: raw.len(),
        });
    }
    let n = view.min(max);
    // SAFETY : `raw` désigne `view` octets lisibles de la mémoire du tableau
    // (QuickJS a vérifié décalage + longueur contre son ArrayBuffer, et la
    // longueur vient d'être comparée à celle de la vue) ; seuls les `n <= view`
    // premiers sont lus, et copiés aussitôt, sans exécuter de JavaScript qui
    // pourrait modifier, détacher ou réallouer le tableau entre-temps.
    let slice = unsafe { std::slice::from_raw_parts(raw.as_ptr().cast::<u8>(), n) };
    Ok(Bytes {
        data: slice.to_vec(),
        total: view,
    })
}

#[cfg(test)]
mod tests {
    use rquickjs::{Context, Runtime};

    use super::*;

    /// Évalue `expr` (un Uint8Array) et copie au plus `max` octets.
    fn eval(expr: &str, max: usize) -> Result<Bytes, BytesError> {
        let rt = Runtime::new().expect("runtime");
        let ctx = Context::full(&rt).expect("contexte");
        ctx.with(|ctx| {
            let array: TypedArray<u8> = ctx.eval(expr).expect("Uint8Array");
            copy(&array, max)
        })
    }

    #[test]
    fn empty_array() {
        assert_eq!(
            eval("new Uint8Array(0)", 16),
            Ok(Bytes {
                data: vec![],
                total: 0
            })
        );
    }

    #[test]
    fn whole_array_within_bound() {
        let b = eval("Uint8Array.of(1, 2, 3)", 16).expect("copie");
        assert_eq!(b.data, vec![1, 2, 3]);
        assert_eq!(b.total, 3);
    }

    #[test]
    fn very_large_array_is_bounded() {
        // 8 Mio : seule la borne est copiée, la longueur totale est rendue.
        let b = eval(
            "const a = new Uint8Array(8 * 1024 * 1024); a[0] = 7; a[4095] = 9; a[4096] = 1; a",
            4096,
        )
        .expect("copie");
        assert_eq!(b.data.len(), 4096);
        assert_eq!((b.data[0], b.data[4095]), (7, 9));
        assert_eq!(b.total, 8 * 1024 * 1024);
    }

    #[test]
    fn partial_view_of_array_buffer() {
        // Vue de 4 octets à partir de l'octet 3 d'un tampon de 10 : 3, 4, 5, 6.
        let b = eval(
            "const buf = new ArrayBuffer(10); new Uint8Array(buf).forEach((_, i, a) => (a[i] = i)); new Uint8Array(buf, 3, 4)",
            16,
        )
        .expect("copie");
        assert_eq!(b.data, vec![3, 4, 5, 6]);
        assert_eq!(b.total, 4);
        // subarray : même chose, bornée à 2.
        let b = eval("Uint8Array.of(9, 8, 7, 6, 5).subarray(1, 4)", 2).expect("copie");
        assert_eq!(b.data, vec![8, 7]);
        assert_eq!(b.total, 3);
    }

    #[test]
    fn detached_array_is_refused() {
        // ArrayBuffer.prototype.transfer détache le tampon d'origine.
        assert_eq!(
            eval(
                "const buf = new ArrayBuffer(8); const a = new Uint8Array(buf); buf.transfer(); a",
                16
            ),
            Err(BytesError::Detached)
        );
    }

    #[test]
    fn zero_bound_copies_nothing() {
        let b = eval("Uint8Array.of(1, 2)", 0).expect("copie");
        assert!(b.data.is_empty());
        assert_eq!(b.total, 2);
    }
}
