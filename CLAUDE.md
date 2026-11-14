# CLAUDE.md — Règles pour l'IA

La référence unique du projet est [docs/cahier-des-charges.md](docs/cahier-des-charges.md). Lis d'abord les sections 1, 2 et 16, puis celle de la tâche. Ce fichier n'en est qu'un résumé : en cas d'écart, le cahier des charges fait foi.

## Contexte

- Prebench simule une carte Nucleo (STM32) ou ESP32 et ses composants. On simule au niveau de l'API (shims), jamais le cœur du microcontrôleur.
- L'utilisateur est ingénieur en électronique embarquée, sur MacBook Apple Silicon avec VS Code. Il est à l'aise en C embarqué et en HTML/CSS/JS.
- Les décisions D1 à D16 (section 2) **ne se rediscutent pas**. Si une demande les contredit, signale-le avant d'agir.
- Précise toujours la phase (section 14) et la section concernée.

## Contrats v1 (phase 0) — toute modification incompatible est une rupture de version

| Contrat | Fichier |
| --- | --- |
| Protocole firmware ↔ app | `docs/protocol.md` |
| API libvmcu | `libvmcu/include/vmcu.h` |
| Interface des composants | `components/component-api.d.ts`, `components/manifest.schema.json` |
| Format des cartes | `boards/board.schema.json` |
| Format de projet | `docs/prebench.schema.json` (`prebench.toml`) |

## Règles d'architecture (16.2)

1. NE DOIT PAS simuler le cœur, l'horloge ou les registres d'un microcontrôleur.
2. Tout appel d'un shim passe par l'API `vmcu_*` ; aucun shim ne parle directement au socket.
3. Tout le temps (millis, micros, délais) passe par libvmcu.
4. Chaque message du protocole porte l'identifiant de carte ; le chemin critique n'utilise jamais de JSON.
5. Les gros flux passent par la mémoire partagée, jamais par le socket.
6. Aucune API système hors de `platform/` (Rust) et `vmcu_platform_*.c` (C).
7. Aucun composant codé en dur dans le cœur : tout composant est un dossier `components/<id>/`.
8. Les modèles de composants n'ont accès ni au disque, ni au réseau, ni au système.
9. Le cœur Rust ne poll jamais : tout est événementiel.
10. Une fonction de framework non simulée compile et avertit ; elle ne provoque jamais d'erreur de lien.
11. Tout tampon est borné.

## Qualité attendue (16.3)

- Code complet, prêt à copier-coller, chemin du fichier en tête.
- Architecture propre et minimale : aucune dépendance ajoutée sans justification.
- Compatible macOS et Windows ; toute différence de comportement est signalée.
- C : C11, compilable avec clang et MinGW-w64, sans extension MSVC.
- Rust : `cargo clippy` sans avertissement ; erreurs typées, aucun `unwrap()` dans le cœur.
- TypeScript : mode strict.
- Tests : chaque shim est accompagné d'un firmware d'exemple qui l'exerce.
- Commentaires et messages de l'interface en français ; identifiants de code en anglais.
- Aucune fonctionnalité absente du cahier des charges sans demande explicite.

## Dépôts et publication (12.5)

- `prebench-dev` (privé) : travail quotidien. `prebench` (public) : un commit « Release vX.Y.Z » par version, publié par `./release.sh vX.Y.Z`.
- Les fichiers internes sont marqués `export-ignore` dans `.gitattributes`. Aucun secret dans le dépôt : clé de l'updater et tokens vont uniquement dans les secrets GitHub.
- CI privée (`ci.yml`) : Linux à chaque push ; Windows quand le code propre au système change, sur un tag, ou à la demande (`gh workflow run ci.yml --ref main`, exigé par `release.sh` avant une release). Release publique (`release.yml`) : macOS arm64 + Windows x64.

## Vérifications locales

```sh
# En-tête C11
printf '#include "vmcu.h"\nint main(void){return 0;}\n' | clang -std=c11 -Wall -Wextra -Wpedantic -Werror -Ilibvmcu/include -x c - -fsyntax-only
# Schémas, cartes, manifestes, projets
python3 .github/scripts/validate_contracts.py      # nécessite jsonschema
# Modèles de composants (TypeScript strict)
npx -p typescript@5.6.3 tsc -p components/tsconfig.json
```
## Interface et design

Avant toute modification de l'interface (app/ui, vues des composants, icônes, couleurs, textes affichés), utilise le skill `prebench-design` : il impose la charte `docs/design.md` et la maquette `docs/design/maquette-ecran-principal.html`. Aucune couleur, taille ou espacement hors charte ; les deux thèmes doivent fonctionner.
