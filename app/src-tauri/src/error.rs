// app/src-tauri/src/error.rs — Erreurs typées du cœur (section 16.3 : pas d'unwrap).

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("erreur d'entrée/sortie : {0}")]
    Io(#[from] std::io::Error),

    #[error("fichier de carte invalide ({path}) : {message}")]
    Board { path: String, message: String },

    #[error("fichier prebench.toml invalide ({path}) : {message}")]
    Project { path: String, message: String },

    #[error("échec du build « sim » :\n{0}")]
    Build(String),

    /// Erreurs du compilateur, détaillées dans l'onglet Build de l'interface.
    #[error("Build en échec : {errors} erreur(s). Voir l'onglet Build pour les corriger.")]
    BuildFailed { errors: usize },

    #[error("ouverture de l'éditeur impossible : {0}")]
    Open(String),

    #[error("réglages : {0}")]
    Settings(String),

    #[error("débogage : {0}")]
    Debug(String),

    #[error("surveillance du dossier du projet impossible : {0}")]
    Watch(String),

    #[error("erreur de protocole : {0}")]
    Protocol(String),

    #[error("le firmware ne s'est pas connecté à temps")]
    ConnectTimeout,

    #[error("aucune simulation en cours")]
    NotRunning,

    #[error("une simulation est déjà en cours")]
    AlreadyRunning,

    #[error("le cœur de simulation s'est arrêté")]
    ActorGone,

    #[error("mise à jour impossible : {0}")]
    Update(String),
}

/// Les commandes Tauri renvoient l'erreur à l'interface sous forme de texte.
impl Serialize for CoreError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
