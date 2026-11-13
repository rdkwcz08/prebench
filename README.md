# Prebench

**Programmer et tester un projet Nucleo (STM32) ou ESP32 avant d'avoir reçu la carte et les composants.**

Prebench est une application de bureau, locale et hors ligne. Elle simule une carte de développement et les composants câblés dessus (LED, boutons, potentiomètres, écrans I2C/SPI…) sur une paillasse virtuelle. Tu gardes ton éditeur habituel : Prebench surveille le dossier du projet, recompile le firmware pour ton ordinateur à chaque sauvegarde et le relance aussitôt.

> **État : développement initial (0.x).** Les contrats de la version 1 sont écrits ; l'application n'est pas encore utilisable.

## Principe

- Le firmware est compilé pour ton ordinateur et lié à **libvmcu**, une bibliothèque qui remplace le HAL (STM32 HAL d'abord, puis ESP-IDF et Arduino). Ton code source ne change pas.
- Le cœur du microcontrôleur, son horloge et ses registres **ne sont pas simulés**. Le simulateur valide la logique ; la vraie carte valide le timing, l'analogique et la physique.
- Les composants sont des dossiers (`components/<id>/` : manifeste, modèle TypeScript, vue Svelte). Toute contribution est bienvenue.
- Tout fonctionne hors ligne, sur macOS (Apple Silicon) et Windows 10/11.

## Téléchargement

Les versions publiées sont dans les [GitHub Releases](https://github.com/rdkwcz08/prebench/releases) :

| Système | Fichier |
| --- | --- |
| macOS (Apple Silicon) | `Prebench_X.Y.Z_aarch64.dmg` |
| Windows 10/11 (x64) | `Prebench_X.Y.Z_x64.msi` |

Une fois installée, l'application se met à jour seule et vérifie la signature de chaque mise à jour.

### Premier lancement sur macOS

L'application n'est pas encore notarisée par Apple : au premier lancement, macOS la bloque.

1. Ouvre Prebench une première fois, puis ferme le message d'avertissement.
2. Ouvre **Réglages Système → Confidentialité et sécurité**.
3. En bas de la page, en face du message sur Prebench, clique **Ouvrir quand même**, puis confirme.

### Premier lancement sur Windows

L'installeur n'est pas encore signé : SmartScreen affiche « Windows a protégé votre ordinateur ».

1. Clique **Informations complémentaires**.
2. Clique **Exécuter quand même**.

## Prérequis utilisateur (simuler un firmware)

Prebench compile ton firmware pour ton ordinateur : il lui faut un compilateur C et CMake 3.20 ou plus récent. Rien d'autre (ni Rust, ni Node).

### macOS

```sh
xcode-select --install   # outils en ligne de commande Xcode : clang
brew install cmake       # ou l'installeur de https://cmake.org/download/
```

Prebench trouve CMake dans `/opt/homebrew/bin`, `/usr/local/bin` ou `/Applications/CMake.app`, même lancé depuis le Finder.

### Windows 10/11

Le firmware se compile avec **MinGW-w64** (gcc), fourni par MSYS2. **MSVC n'est pas pris en charge** pour le firmware : le code HAL utilise des extensions GCC.

Dans PowerShell :

```powershell
# 1. MSYS2 et CMake (ADD_CMAKE_TO_PATH : l'installateur de CMake n'ajoute pas CMake au PATH par défaut)
winget install --id MSYS2.MSYS2 -e
winget install --id Kitware.CMake -e --custom "ADD_CMAKE_TO_PATH=System"

# 2. gcc et make de MinGW-w64 (environnement UCRT64)
C:\msys64\usr\bin\bash.exe -lc "pacman -S --needed --noconfirm mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-make"

# 3. Ajouter gcc au PATH de l'utilisateur, puis rouvrir PowerShell
[Environment]::SetEnvironmentVariable("Path", [Environment]::GetEnvironmentVariable("Path", "User") + ";C:\msys64\ucrt64\bin", "User")

# 4. Vérification (dans un nouveau PowerShell)
gcc --version; mingw32-make --version; cmake --version
```

CMake déjà installé sans l'option PATH : rien à refaire, Prebench le trouve dans `C:\Program Files\CMake\bin`, et gcc dans `C:\msys64\ucrt64\bin`. Pour l'utiliser aussi dans un terminal, réinstalle-le avec l'option : `winget install --id Kitware.CMake -e --force --custom "ADD_CMAKE_TO_PATH=System"`.

Au démarrage, Prebench vérifie ces prérequis : s'il en manque un, un bandeau donne la commande à lancer. Les chemins des outils utilisés sont écrits dans le Journal à chaque ▶ Démarrer.

## Premier projet

1. Au premier lancement, clique **Créer mon premier projet**, puis **Créer** : Prebench copie le modèle `blinky-stm32-hal` dans `Documents/Prebench/blinky-stm32-hal`, un dossier que tu peux modifier. Plus tard, **Nouveau projet…** fait de même avec le modèle de ton choix.
2. Clique **Démarrer** : LD2 clignote, le bouton B1 change la vitesse.
3. Modifie `Core/Src/main.c` dans ton éditeur et enregistre : Prebench recompile et recharge le firmware, sans toucher à la paillasse. Une erreur de compilation s'affiche dans l'onglet **Build** ; un clic sur `fichier:ligne` l'ouvre dans VS Code.

Le firmware est compilé dans le cache de Prebench, jamais dans le dossier du projet : `~/Library/Caches/dev.prebench.app/builds` (macOS), `%LOCALAPPDATA%\dev.prebench.app\builds` (Windows). Une erreur de compilation garde ce build, pour que la correction recompile seulement le fichier modifié. Ce dossier peut être supprimé à tout moment.

## Déboguer le firmware

Le firmware se débogue dans VS Code, avec des points d'arrêt dans ton code C ; Prebench affiche la paillasse.

| Système | À installer |
| --- | --- |
| macOS | Extension **CodeLLDB** : `code --install-extension vadimcn.vscode-lldb` (lldb est fourni avec l'extension) |
| Windows | Extension **C/C++** de Microsoft : `code --install-extension ms-vscode.cpptools`, et gdb de MSYS2 : `C:\msys64\usr\bin\bash.exe -lc "pacman -S --needed --noconfirm mingw-w64-ucrt-x86_64-gdb"` |
| Linux | Extension **C/C++** de Microsoft et `sudo apt install gdb` |

1. Dans Prebench, clique **Déboguer** : le firmware est compilé et la configuration « Prebench : déboguer le firmware » est ajoutée à `.vscode/launch.json` du projet (les autres configurations sont conservées ; les commentaires du fichier ne le sont pas). Prebench signale une extension ou un débogueur absent, avec la commande pour l'installer.
2. Dans VS Code, choisis cette configuration et lance le débogage (F5).
3. Sur un point d'arrêt, la paillasse se fige et la barre d'état affiche « Arrêté sur un point d'arrêt ». À la reprise, le temps du firmware repart sans saut : un `HAL_Delay` en cours n'est pas raccourci.
4. Une sauvegarde recompile le firmware ; il sert à la session suivante (Maj+F5, puis F5). **Arrêter** dans Prebench termine la session.

Le firmware est compilé avec les informations de débogage, sans optimisation. Pour IntelliSense (extension C/C++), Prebench inscrit dans `.vscode/settings.json` le `compile_commands.json` du build : les en-têtes du shim (`stm32f4xx_hal.h`…) sont trouvés. Les autres réglages du fichier sont conservés.

### Facultatif

Pour la jauge mémoire de la vraie cible : `arm-none-eabi-gcc` (STM32), toolchain Xtensa d'ESP-IDF (ESP32).

## Prérequis développeur (construire Prebench)

Seulement pour travailler sur Prebench lui-même ; les utilisateurs installent le `.dmg` ou le `.msi`.

| Outil | macOS | Windows |
| --- | --- | --- |
| Prérequis utilisateur ci-dessus | ✔ | ✔ |
| Rust (rustup, avec clippy et rustfmt) | ✔ | ✔ |
| Node.js LTS | ✔ | ✔ |
| Visual Studio Build Tools, charge « Développement Desktop en C++ » (cible Rust MSVC de Tauri) | — | ✔ |
| WebView2 | intégré (WebKit) | intégré à Windows 11 |

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal -c clippy,rustfmt
```

La CLI Tauri n'est pas installée globalement : c'est une dépendance de développement de `app/package.json`, installée par `npm ci`.

Lancer l'app en développement (rechargement à chaud de l'interface) :

```sh
cd app
npm ci
npm run tauri dev
```

Au démarrage, l'app ouvre la copie du blinky dans `Documents/Prebench` si elle existe, sinon elle propose de la créer. Pour travailler directement sur le modèle du dépôt, saisis le chemin de `templates/blinky-stm32-hal` et clique **Ouvrir**. **▶ Démarrer** compile le projet (cible « sim ») dans le cache de l'app et le lance.

Tests du cœur Rust, dont deux avec le vrai firmware compilé (blinky, mesure de latence) :

```sh
cd app/src-tauri
cargo test -- --nocapture --test-threads=1
```

Tests de libvmcu et du shim STM32 HAL :

```sh
cmake -S libvmcu -B build-libvmcu -DVMCU_WERROR=ON
cmake --build build-libvmcu --parallel
ctest --test-dir build-libvmcu --output-on-failure
```

## Documentation

- [Protocole firmware ↔ app](docs/protocol.md)
- [API libvmcu](libvmcu/include/vmcu.h)
- [Interface des composants](components/component-api.d.ts) et [schéma des manifestes](components/manifest.schema.json)
- [Format des cartes](boards/board.schema.json) et [format de `prebench.toml`](docs/prebench.schema.json)
- [Cahier des charges](docs/cahier-des-charges.md)

## Contribuer

Les pull requests sont les bienvenues sur ce dépôt, en particulier pour les nouveaux composants (`components/<id>/`) et les nouvelles cartes (`boards/<id>.toml`). Commentaires et messages de l'interface sont en français ; les identifiants de code sont en anglais.

## Licence

[MIT](LICENSE) pour tout le projet : application, libvmcu, shims, cartes et composants.
