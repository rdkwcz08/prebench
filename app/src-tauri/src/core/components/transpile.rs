// app/src-tauri/src/core/components/transpile.rs — model.ts -> JavaScript (oxc).
//
// Retire uniquement les types, fichier par fichier (équivalent de
// isolatedModules) : aucune vérification de types ici, elle se fait dans
// l'éditeur et en CI (tsc strict). Aucun outil externe : l'app installée
// compile elle-même les composants ajoutés par l'utilisateur (D4).

use std::path::Path;

use oxc::allocator::Allocator;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};

/// Compile une source TypeScript (ou JavaScript) en module ES.
pub fn strip_types(path: &Path, source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if let Some(error) = parsed.diagnostics.first() {
        return Err(format!("{} : {error}", path.display()));
    }
    let mut program = parsed.program;
    let semantic = SemanticBuilder::new().build(&program);
    let scoping = semantic.semantic.into_scoping();
    let transformed = Transformer::new(&allocator, path, &TransformOptions::default())
        .build_with_scoping(scoping, &mut program);
    if let Some(error) = transformed.diagnostics.first() {
        return Err(format!("{} : {error}", path.display()));
    }
    Ok(Codegen::new().build(&program).code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_types_and_type_imports() {
        let src = "import type { ComponentFactory } from '../component-api';\n\
                   type S = { on: boolean };\n\
                   const f: ComponentFactory<object, S> = (ctx) => ({ getState: (): S => ({ on: true }) });\n\
                   export default f;\n";
        let js = strip_types(Path::new("model.ts"), src).expect("transpilation");
        assert!(
            !js.contains("component-api"),
            "import de types retiré : {js}"
        );
        assert!(!js.contains(": S"), "annotations retirées : {js}");
        assert!(js.contains("export default f"));
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(strip_types(Path::new("model.ts"), "const = ;").is_err());
    }
}
