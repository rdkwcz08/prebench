#!/bin/sh
# app/src-tauri/icons/regenerer.sh — Régénère les icônes de l'app : .icns, .ico et PNG
# depuis source/icon-1024.png, et vérifie que source/prebench.icon se compile en un
# Assets.car complet (le build de l'app le compile lui-même : il n'est pas commité).
# Procédure et explications : README.md du même dossier. Outil de mainteneur, macOS seulement
# (sips, et actool de Xcode 26 ou plus pour Assets.car).
set -eu

ICONS="$(cd "$(dirname "$0")" && pwd)"
APP="$(cd "$ICONS/../.." && pwd)"
SRC="$ICONS/source/icon-1024.png"
ICON_COMPOSER="$ICONS/source/prebench.icon"
# Version minimale de macOS de l'app (LSMinimumSystemVersion, défaut de Tauri) : actool y
# ajoute des images figées pour les macOS antérieurs à 26.
MIN_MACOS="10.13"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

[ -f "$SRC" ] || { echo "Source absente : $SRC" >&2; exit 1; }
[ -d "$ICON_COMPOSER" ] || { echo "Source absente : $ICON_COMPOSER" >&2; exit 1; }
ACTOOL_MAJOR="$(xcrun actool --version --output-format=human-readable-text 2>/dev/null \
  | sed -n 's/.*short-bundle-version: *\([0-9][0-9]*\).*/\1/p')"
[ "${ACTOOL_MAJOR:-0}" -ge 26 ] || {
  echo "actool de Xcode 26 ou plus introuvable ($(xcode-select -p 2>/dev/null))." >&2
  echo "Installer Xcode puis : sudo xcode-select -s /Applications/Xcode.app/Contents/Developer" >&2
  exit 1
}
[ "$(sips -g pixelWidth -g pixelHeight "$SRC" | awk '/pixel/ { print $2 }' | sort -u)" = "1024" ] \
  || { echo "La source doit faire 1024 × 1024 px" >&2; exit 1; }

# 1. PNG et .ico (Windows) : image pleine, la forme occupe tout le carré.
(cd "$APP" && npx tauri icon "$SRC" -o "$TMP/plein" >/dev/null)

# 2. .icns (macOS antérieurs à 26) : grille Apple, forme de 824 px centrée dans 1024 px,
#    pour que l'icône ait la même taille que les autres dans le Dock et le Finder.
sips -z 824 824 "$SRC" --out "$TMP/824.png" >/dev/null
sips -p 1024 1024 "$TMP/824.png" --out "$TMP/grille.png" >/dev/null
(cd "$APP" && npx tauri icon "$TMP/grille.png" -o "$TMP/grille" >/dev/null)

# 3. Assets.car (macOS 26 et plus, icône qui suit l'apparence) : compilation d'essai du
#    projet Icon Composer, comme le fera Tauri au build, pour vérifier ses variantes.
cp -R "$ICON_COMPOSER" "$TMP/AppIcon.icon"
mkdir -p "$TMP/car"
xcrun actool "$TMP/AppIcon.icon" --compile "$TMP/car" \
  --output-format human-readable-text --notices --warnings \
  --output-partial-info-plist "$TMP/car/partial-info.plist" \
  --app-icon AppIcon --include-all-app-icons \
  --enable-on-demand-resources NO --development-region fr \
  --target-device mac --platform macosx --minimum-deployment-target "$MIN_MACOS"
"$ICONS/verifier-assets-car.sh" "$TMP/car/Assets.car" AppIcon

# Les dossiers android/ et ios/ produits par tauri icon ne sont pas copiés (cibles hors projet).
cp "$TMP"/plein/*.png "$TMP/plein/icon.ico" "$ICONS/"
cp "$TMP/grille/icon.icns" "$ICONS/"
echo "Icônes régénérées dans $ICONS ; source/prebench.icon vérifié avec actool $(xcrun actool \
  --version --output-format=human-readable-text | sed -n 's/.*short-bundle-version: *//p' | head -n 1)"
