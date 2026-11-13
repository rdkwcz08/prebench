// app/src-tauri/src/build/diagnostics.rs — Erreurs et avertissements d'un build « sim ».
//
// Lit la sortie de CMake, du compilateur (gcc, clang) et de l'éditeur de liens
// pour en tirer une liste `fichier:ligne:colonne` cliquable (section 10.2).
// Formats reconnus :
//   main.c:12:5: error: …            (gcc, clang ; aussi « fatal error », « warning »)
//   C:\proj\main.c:12:5: error: …    (Windows : le « C: » n'est pas un séparateur)
//   main.c:12: error: …              (sans colonne)
//   CMake Error at CMakeLists.txt:7 (add_executable):   (message sur les lignes suivantes)
//   … undefined reference to `foo'  (ld GNU) ; Undefined symbols … (ld64) : sans emplacement
// Une sortie non reconnue ne donne aucune entrée : le Journal garde le texte complet.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Nombre maximal d'entrées gardées (règle 16.2.11).
pub const MAX_DIAGNOSTICS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Chemin absolu du fichier, s'il est connu.
    pub file: Option<PathBuf>,
    /// Chemin affiché : relatif au projet quand le fichier y est.
    pub display: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
}

/// Marqueurs de sévérité, du plus long au plus court.
const MARKERS: &[(&str, Severity)] = &[
    (": fatal error: ", Severity::Error),
    (": error: ", Severity::Error),
    (": warning: ", Severity::Warning),
];

/// Analyse la sortie d'un build ; `project` sert à résoudre et raccourcir les chemins.
pub fn parse(output: &str, project: &Path) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let mut lines = output.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim_end();
        let found = if let Some(rest) = line.strip_prefix("CMake Error at ") {
            // Message CMake : lignes indentées qui suivent, jusqu'à une ligne vide.
            let mut message = Vec::new();
            while let Some(next) = lines.peek() {
                let t = next.trim();
                if t.is_empty() && !message.is_empty() {
                    break;
                }
                if !t.is_empty() {
                    message.push(t.to_string());
                }
                lines.next();
                if message.len() >= 6 {
                    break;
                }
            }
            cmake_error(rest, message.join(" "), project)
        } else if let Some(d) = located(line, project) {
            Some(d)
        } else {
            unlocated(line)
        };
        if let Some(d) = found {
            if !out.contains(&d) {
                out.push(d);
            }
            if out.len() >= MAX_DIAGNOSTICS {
                break;
            }
        }
    }
    out
}

/// `fichier:ligne[:colonne]: error: message`
fn located(line: &str, project: &Path) -> Option<Diagnostic> {
    let (pos, marker, severity) = MARKERS
        .iter()
        .find_map(|(m, s)| line.find(m).map(|p| (p, *m, *s)))?;
    let head = &line[..pos];
    let message = line[pos + marker.len()..].trim().to_string();
    // Depuis la droite : colonne et ligne numériques, le reste est le chemin.
    let mut parts = head.rsplitn(3, ':');
    let a = parts.next()?;
    let b = parts.next();
    let c = parts.next();
    let (file, line_no, column) = match (a.parse::<u32>(), b.map(str::parse::<u32>), c) {
        (Ok(col), Some(Ok(ln)), Some(f)) => (f, ln, Some(col)),
        (Ok(ln), Some(Err(_)), _) | (Ok(ln), None, _) => {
            // Pas de colonne : « fichier:ligne ». Le chemin est tout ce qui précède.
            let f = head.rsplit_once(':').map(|(f, _)| f)?;
            (f, ln, None)
        }
        _ => return None,
    };
    let file = file.trim();
    if file.is_empty() {
        return None;
    }
    Some(with_location(
        severity, file, line_no, column, message, project,
    ))
}

fn cmake_error(rest: &str, message: String, project: &Path) -> Option<Diagnostic> {
    // « CMakeLists.txt:7 (add_executable): »
    let location = rest.trim_end_matches(':');
    let location = location.split(" (").next().unwrap_or(location);
    let (file, line_no) = location.rsplit_once(':')?;
    let line_no = line_no.parse().ok()?;
    let message = if message.is_empty() {
        "erreur de configuration CMake".to_string()
    } else {
        message
    };
    Some(with_location(
        Severity::Error,
        file,
        line_no,
        None,
        message,
        project,
    ))
}

/// Erreurs de l'éditeur de liens, sans emplacement utilisable.
fn unlocated(line: &str) -> Option<Diagnostic> {
    let t = line.trim();
    let message = if let Some(i) = t.find("undefined reference to ") {
        Some(t[i..].to_string())
    } else if t.starts_with("Undefined symbols for architecture") {
        Some(t.trim_end_matches(':').to_string())
    } else if t.contains("multiple definition of ") {
        t.find("multiple definition of ")
            .map(|i| t[i..].to_string())
    } else {
        None
    }?;
    Some(Diagnostic {
        severity: Severity::Error,
        file: None,
        display: None,
        line: None,
        column: None,
        message,
    })
}

fn with_location(
    severity: Severity,
    file: &str,
    line: u32,
    column: Option<u32>,
    message: String,
    project: &Path,
) -> Diagnostic {
    let path = Path::new(file);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project.join(path)
    };
    let display = abs
        .strip_prefix(project)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file.replace('\\', "/"));
    Diagnostic {
        severity,
        file: Some(abs),
        display: Some(display),
        line: Some(line),
        column,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"C:\Users\demo\blinky")
        } else {
            PathBuf::from("/Users/demo/blinky")
        }
    }

    #[test]
    fn gcc_and_clang_errors() {
        let p = project();
        let src = p.join("Core").join("Src").join("main.c");
        let out = format!(
            "[ 50%] Building C object …\n\
             {f}:42:5: error: expected ';' after expression\n\
             {f}:12:10: warning: unused variable 'x' [-Wunused-variable]\n\
             {f}:42:5: error: expected ';' after expression\n\
             {f}:3:10: fatal error: 'missing.h' file not found\n\
             2 errors generated.\n",
            f = src.display()
        );
        let d = parse(&out, &p);
        assert_eq!(d.len(), 3, "{d:?}");
        assert_eq!(d[0].severity, Severity::Error);
        assert_eq!(d[0].display.as_deref(), Some("Core/Src/main.c"));
        assert_eq!((d[0].line, d[0].column), (Some(42), Some(5)));
        assert_eq!(d[0].message, "expected ';' after expression");
        assert_eq!(d[1].severity, Severity::Warning);
        assert_eq!(d[2].message, "'missing.h' file not found");
        assert_eq!(d[2].file.as_deref(), Some(src.as_path()));
    }

    #[test]
    fn windows_drive_letter_is_not_a_separator() {
        let d = parse(
            r"C:\Users\demo\blinky\Core\Src\main.c:7:3: error: 'foo' undeclared",
            Path::new(r"C:\Users\demo\blinky"),
        );
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].line, d[0].column), (Some(7), Some(3)));
        assert!(d[0]
            .file
            .as_ref()
            .is_some_and(|f| f.to_string_lossy().starts_with(r"C:\Users")));
    }

    #[test]
    fn without_column_relative_and_outside_project() {
        let p = project();
        let d = parse("main.c:9: error: oops\n/opt/sdk/vmcu.h:3:1: warning: w", &p);
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].line, d[0].column), (Some(9), None));
        assert_eq!(d[0].file.as_deref(), Some(p.join("main.c").as_path()));
        assert_eq!(d[1].display.as_deref(), Some("/opt/sdk/vmcu.h"));
    }

    #[test]
    fn cmake_and_linker_errors() {
        let p = project();
        let out = "CMake Error at CMakeLists.txt:7 (add_executable):\n  Cannot find source file:\n\n    nope.c\n\n\
                   /usr/bin/ld: main.c:(.text+0x1c): undefined reference to `HAL_Foo'\n\
                   Undefined symbols for architecture arm64:\n";
        let d = parse(out, &p);
        assert_eq!(d.len(), 3, "{d:?}");
        assert_eq!(d[0].display.as_deref(), Some("CMakeLists.txt"));
        assert_eq!(d[0].line, Some(7));
        assert_eq!(d[0].message, "Cannot find source file:");
        assert_eq!(d[1].file, None);
        assert_eq!(d[1].message, "undefined reference to `HAL_Foo'");
        assert_eq!(d[2].message, "Undefined symbols for architecture arm64");
    }

    #[test]
    fn noise_is_ignored_and_output_is_bounded() {
        assert!(parse(
            "make[2]: *** [all] Error 2\n-- Configuring done",
            &project()
        )
        .is_empty());
        let many: String = (0..500)
            .map(|i| format!("a.c:{i}:1: error: e{i}\n"))
            .collect();
        assert_eq!(parse(&many, &project()).len(), MAX_DIAGNOSTICS);
    }
}
