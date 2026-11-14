# Icônes de l'app

## Sources

| Fichier | Rôle |
| --- | --- |
| `source/prebench.icon` | Projet Icon Composer : l'icône Liquid Glass (macOS 26 et plus), compilée en `Assets.car` par le build de l'app. |
| `source/icon-1024.png` | Export macOS d'Icon Composer, apparence par défaut, 1024 × 1024. Source de toutes les autres tailles. |

Tout le reste du dossier est généré et commité : ne pas le retoucher à la main. `Assets.car` n'est pas commité : Tauri le compile à chaque build de l'app à partir de `source/prebench.icon` (voir plus bas).

## Fichiers générés et usage

| Fichier | Utilisé par |
| --- | --- |
| `Assets.car` (dans l'app, compilé au build) | macOS 26 et plus : icône qui suit l'apparence (claire, sombre, teintée, transparente), désignée par `CFBundleIconName` |
| `icon.ico` (16, 24, 32, 48, 64, 256 px) | Windows : exécutable, installeur .msi, barre des tâches, menu Démarrer, Explorateur |
| `icon.icns` | Repli (`CFBundleIconFile`) pour les macOS antérieurs à 26 |
| `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png` | Icône de fenêtre et paquets Linux |
| `Square*Logo.png`, `StoreLogo.png` | Paquets Windows du Store (non utilisés aujourd'hui, produits par `tauri icon`) |

Le `.icns` est fait sur la grille Apple (forme de 824 px centrée dans 1024 px), pour avoir la taille des autres icônes du Dock. Le `.ico` et les PNG utilisent l'image pleine, puisque Windows n'ajoute pas de marge.

## Régénérer

1. Dans Icon Composer, ouvrir `source/prebench.icon`, faire les modifications et enregistrer.
2. Exporter l'apparence par défaut pour macOS en PNG 1024 × 1024, et remplacer `source/icon-1024.png`.
3. Depuis la racine du dépôt :

   ```sh
   app/src-tauri/icons/regenerer.sh
   ```

   Le script lance `npx tauri icon` deux fois (image pleine, puis grille Apple pour le `.icns`) et copie les fichiers ici, sauf les dossiers `android/` et `ios/`, hors projet. Il compile aussi `source/prebench.icon` à titre d'essai avec `actool` et vérifie ses variantes (`verifier-assets-car.sh`), sans garder le résultat. Il ne tourne que sur macOS, avec Xcode 26 ou plus (voir plus bas).

4. Contrôler la lisibilité à 16 px (voir plus bas), puis commiter les sources et les fichiers générés ensemble.

## macOS 26 : `Assets.car`

`tauri.conf.json` liste `icons/source/prebench.icon` dans `bundle.icon`. Au build d'un paquet macOS, Tauri (bundler 2.10.4) le compile en `Assets.car` avec `actool`, le place dans `Contents/Resources/` de l'app, y lit le nom de l'icône avec `assetutil` et l'écrit dans `CFBundleIconName`. `CFBundleIconFile` désigne toujours `icon.icns`. Windows et Linux ignorent ce projet (ils utilisent `icon.ico` et les PNG).

- **Sans Xcode 26** (outils en ligne de commande seuls, Xcode plus ancien) : le build passe, Tauri affiche « failed to get actool version, skipping Assets.car file creation » et l'app n'a que le `.icns`, sans les variantes de macOS 26. C'est le cas d'un build local sans Xcode ; c'est sans conséquence pour le développement.
- **Release** : `release.yml` sélectionne explicitement le Xcode 26 ou plus le plus récent du runner macOS (échec clair s'il n'y en a pas), compile lui-même `source/prebench.icon` en `Assets.car` avec `actool`, vérifie ses variantes (`verifier-assets-car.sh`), puis le donne à Tauri par `--config` (liste d'icônes de `tauri.conf.json` où l'`Assets.car` remplace le `.icon`) ; Tauri le copie tel quel. La compilation par Tauri lui-même plantait dans `actool` sur le runner (exception interne, release v0.2.1) alors que la même commande lancée seule réussit ; la cause n'est pas identifiée. L'`Assets.car` produit n'est jamais commité (`.gitignore`).

- **Xcode requis pour régénérer, et pour un paquet local avec l'icône Liquid Glass** : Xcode 26 ou plus, complet (les outils en ligne de commande ne contiennent pas `actool`). Après l'installation : `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`, puis `xcrun actool --version`.
- **Macs antérieurs à macOS 26** : `actool` est appelé avec `--minimum-deployment-target 10.13` (la version minimale de l'app), pour ajouter à `Assets.car` des images figées à leur intention.
- **`npx tauri dev`** : l'app n'est pas un vrai paquet `.app`, l'icône ne suit donc pas l'apparence. Pour vérifier, il faut un paquet : `npx tauri build --bundles app`.

Dans `release.yml`, l'étape « Vérification de l'app installée (macOS) » exige, pour l'app copiée depuis le `.dmg` : un `Assets.car` avec une icône nommée comme `CFBundleIconName` et les variantes claire (`NSAppearanceNameAqua`), sombre (`NSAppearanceNameDarkAqua`) et teintée (`ISAppearanceTintable`, d'où dérive aussi le mode transparent), et la présence du `.icns`. Le rendu à l'écran n'est pas contrôlé par la CI.

## Contrôles de rendu

- **16 px** : la taille qu'utilisent la barre de titre de Windows, l'Explorateur en petites icônes et la liste du Finder. « Pre », en blanc sur bleu clair, y est peu contrasté : il doit rester reconnaissable en mode clair comme en mode sombre.
- **macOS** : sur un paquet construit (`npx tauri build --bundles app`), Dock et Finder (icônes, liste, Lire les informations). Sur macOS 26 et plus, l'icône doit changer en passant Réglages Système → Apparence → Style des icônes en sombre, transparent puis teinté. Si possible, vérifier aussi sur un macOS antérieur. Le cache d'icônes peut garder l'ancienne image : `killall Dock Finder` après installation.
- **Windows** : barre des tâches à 100 % et à 150 % (24 et 32 px), menu Démarrer, raccourci du Bureau, Paramètres → Applications. Le cache d'icônes de Windows garde parfois l'ancienne icône après une mise à jour : `ie4uinit.exe -show` pour le vider.
