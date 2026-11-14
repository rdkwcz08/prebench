#!/bin/sh
# app/src-tauri/icons/verifier-assets-car.sh — Vérifie qu'un Assets.car contient une icône d'app
# qui suit l'apparence de macOS 26 et plus (claire, sombre, teintée ou transparente).
# Usage : verifier-assets-car.sh <Assets.car> [nom attendu]
# Utilisé par regenerer.sh et par release.yml. macOS seulement (assetutil, livré avec le système).
set -eu

CAR="$1"
EXPECTED="${2:-}"
[ -f "$CAR" ] || { echo "Assets.car absent : $CAR" >&2; exit 1; }

assetutil --info "$CAR" | python3 -I -c '
import json, sys
expected = sys.argv[1]
entries = json.load(sys.stdin)[1:]
icons = sorted({e["Name"] for e in entries if e.get("AssetType") == "Icon Image"})
if len(icons) != 1:
    sys.exit(f"Une seule icône d’app attendue dans Assets.car, trouvé : {icons}")
name = icons[0]
if expected and name != expected:
    sys.exit(f"Icône « {name} » dans Assets.car, mais CFBundleIconName vaut « {expected} »")
stacks = {e.get("Appearance") for e in entries
          if e.get("AssetType") == "IconImageStack" and e.get("Name") == name}
# Teintée et transparente reposent sur la variante ISAppearanceTintable.
missing = {"NSAppearanceNameAqua", "NSAppearanceNameDarkAqua", "ISAppearanceTintable"} - stacks
if missing:
    sys.exit(f"Icône « {name} » : variantes absentes {sorted(missing)} (présentes : {sorted(map(str, stacks))})")
print(f"Assets.car : icône « {name} », variantes {sorted(stacks)}")
' "$EXPECTED"
