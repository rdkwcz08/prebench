// app/src-tauri/src/core/environment.rs — Grandeurs du panneau « environnement »
// (section 8.5 ; component-api.d.ts, Environment).
//
// Réglées dans l'interface, gardées pendant la session de l'app (rechargement
// à chaud et redémarrage de la simulation compris), jamais écrites dans
// prebench.toml (sauvegarde par projet : Plus tard). Chaque valeur est bornée ;
// une valeur absente ou non finie reprend sa valeur par défaut.
// L'accélération n'est pas réglée directement : c'est la gravité vue par un
// capteur incliné de roll et pitch (1 g au repos, à plat), comme un IMU posé
// sur la paillasse.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tilt {
    pub roll: f64,
    pub pitch: f64,
}

impl Default for Tilt {
    fn default() -> Self {
        Self {
            roll: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Environment {
    pub temperature_c: f64,
    pub humidity_percent: f64,
    pub illuminance_lux: f64,
    pub tilt_deg: Tilt,
    /// Calculée depuis tilt_deg (voir `settle`), jamais lue de l'interface.
    pub acceleration_g: Vec3,
    pub distance_m: f64,
}

impl Default for Environment {
    /// Pièce de travail ordinaire : 22 °C, 50 %, 300 lx, à plat, 1 m.
    fn default() -> Self {
        Self {
            temperature_c: 22.0,
            humidity_percent: 50.0,
            illuminance_lux: 300.0,
            tilt_deg: Tilt::default(),
            acceleration_g: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            distance_m: 1.0,
        }
    }
}

/// Valeur bornée ; non finie : la valeur par défaut.
fn bound(v: f64, min: f64, max: f64, default: f64) -> f64 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        default
    }
}

impl Environment {
    /// Bornes de chaque grandeur, puis accélération déduite de l'inclinaison.
    pub fn settle(self) -> Self {
        let d = Self::default();
        let roll = bound(self.tilt_deg.roll, -180.0, 180.0, 0.0);
        let pitch = bound(self.tilt_deg.pitch, -90.0, 90.0, 0.0);
        let (r, p) = (roll.to_radians(), pitch.to_radians());
        // Gravité dans le repère du capteur (x vers l'avant, z vers le haut).
        let round = |v: f64| (v * 1e6).round() / 1e6;
        Self {
            temperature_c: bound(self.temperature_c, -40.0, 85.0, d.temperature_c),
            humidity_percent: bound(self.humidity_percent, 0.0, 100.0, d.humidity_percent),
            illuminance_lux: bound(self.illuminance_lux, 0.0, 100_000.0, d.illuminance_lux),
            tilt_deg: Tilt { roll, pitch },
            acceleration_g: Vec3 {
                x: round(-p.sin()),
                y: round(r.sin() * p.cos()),
                z: round(r.cos() * p.cos()),
            },
            distance_m: bound(self.distance_m, 0.02, 4.0, d.distance_m),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_flat_and_still() {
        let e = Environment::default().settle();
        assert_eq!(e, Environment::default());
        assert_eq!(
            e.acceleration_g,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0
            }
        );
    }

    #[test]
    fn bounds_and_gravity_from_tilt() {
        let e: Environment = serde_json::from_value(serde_json::json!({
            "temperatureC": 500.0,
            "humidityPercent": -3,
            "tiltDeg": { "roll": 90.0, "pitch": 0.0 },
            "accelerationG": { "x": 9, "y": 9, "z": 9 },
        }))
        .expect("JSON");
        let e = e.settle();
        assert_eq!(e.temperature_c, 85.0);
        assert_eq!(e.humidity_percent, 0.0);
        assert_eq!(e.illuminance_lux, 300.0); // absent : défaut
                                              // Roulis de 90° : la gravité passe sur y ; l'accélération envoyée est ignorée.
        assert_eq!(
            e.acceleration_g,
            Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            }
        );
        let nose_up = Environment {
            tilt_deg: Tilt {
                roll: 0.0,
                pitch: 30.0,
            },
            ..Environment::default()
        }
        .settle();
        assert_eq!(nose_up.acceleration_g.x, -0.5);
        let json = serde_json::to_value(nose_up).expect("JSON");
        assert!(json.get("tiltDeg").is_some() && json.get("accelerationG").is_some());
    }
}
