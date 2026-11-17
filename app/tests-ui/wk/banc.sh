#!/bin/sh
# app/tests-ui/wk/banc.sh — Mesure du rendu de tests/budget dans le vrai WKWebView (macOS).
#
# Prérequis : app/src-tauri/target/budget-events.json (test budget_core_profile) et
# l'interface construite (npx vite build, dans app/). Usage, depuis app/ :
#   sh tests-ui/wk/banc.sh [taux d'images/s]
# Écrit le CPU (% d'un cœur) et la mémoire du programme et de ses processus WebKit
# (WebContent, GPU, réseau) pendant le rejeu. Ne pas toucher la fenêtre pendant la mesure.
# Rejeu natif par défaut, comme Tauri (evaluateJavaScript, un sim:frame par image) ;
# BANC_JS=1 : ancien rejeu dans la page. Fenêtre de 1200 × 800 points comme l'app
# (BANC_LARGEUR, BANC_HAUTEUR). BANC_DIAG=1 : relevé des images et du DOM de la page
# (diag-page.js), comme la version de diagnostic de l'app.
set -eu
export LC_ALL=C
RATE="${1:-24.5}"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$(cd "$HERE/../.." && pwd)"
WORK="$(mktemp -d)"
PREVIEW=""
BANC=""
# npx lance node dans un processus enfant : arrêté par son port, sinon un
# ancien serveur resterait et servirait une liste de fichiers périmée.
stop_preview() { pkill -f "vite preview --host 127.0.0.1 --port 4174" 2>/dev/null || true; }
trap 'kill ${PREVIEW:-} ${BANC:-} 2>/dev/null || true; stop_preview; rm -rf "$WORK"' EXIT
if curl -sf http://127.0.0.1:4174 >/dev/null 2>&1; then
  echo "Port 4174 déjà pris (ancien banc ?) : arrêtez ce serveur avant la mesure." >&2
  exit 1
fi

(cd "$APP" && PREBENCH_PERF_EXPORT=1 PREBENCH_PERF_RATE="$RATE" npx playwright test perf -g export \
  --project=chromium >/dev/null)
[ "${BANC_DIAG:-0}" = "1" ] && cat "$HERE/diag-page.js" >> "$APP/test-results/wk-init.js"
swiftc -O "$HERE/banc.swift" -o "$WORK/banc" 2>/dev/null
cat > "$WORK/resp.c" <<'C'
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
extern pid_t responsibility_get_pid_responsible_for_pid(pid_t);
int main(int argc, char **argv) {
    for (int i = 1; i < argc; ++i) printf("%s %d\n", argv[i], (int)responsibility_get_pid_responsible_for_pid((pid_t)atoi(argv[i])));
    return 0;
}
C
cc -o "$WORK/resp" "$WORK/resp.c"

(cd "$APP" && npx vite preview --host 127.0.0.1 --port 4174 --strictPort >/dev/null 2>&1) &
PREVIEW=$!
until curl -sf http://127.0.0.1:4174 >/dev/null; do sleep 0.2; done

"$WORK/banc" http://127.0.0.1:4174/ "$APP/test-results/wk-init.js" $([ "${BANC_JS:-0}" = "1" ] || echo "$APP/test-results/wk-events.json") > "$WORK/out.txt" &
BANC=$!
until grep -q "DEBUT" "$WORK/out.txt" 2>/dev/null; do sleep 0.1; done

# Processus WebKit du banc : même processus responsable que lui (le terminal,
# pour un exécutable non empaqueté) et lancés après lui.
started() { date -j -f "%a %b %d %T %Y" "$(ps -o lstart= -p "$1" | xargs)" +%s 2>/dev/null || echo 0; }
RESP="$("$WORK/resp" "$BANC" | awk '{ print $2 }')"
BANC_START="$(started "$BANC")"
pids="$BANC"
for p in $("$WORK/resp" $(pgrep -f com.apple.WebKit) | awk -v r="$RESP" '$2 == r { print $1 }'); do
  [ "$(started "$p")" -ge "$BANC_START" ] && pids="$pids $p"
done
cputime() { ps -o time= -p "$1" 2>/dev/null | awk -F: '{ n = split($0, a, ":"); s = 0; for (i = 1; i <= n; i++) s = s * 60 + a[i]; printf "%d\n", s * 100 }'; }
for p in $pids; do eval "t0_$p=\$(cputime $p)"; done
T0=$(date +%s)
until grep -qE "FIN|ERREUR" "$WORK/out.txt"; do sleep 0.2; done
SECONDS_=$(( $(date +%s) - T0 ))
[ "$SECONDS_" -gt 0 ] || SECONDS_=1
total=0
for p in $pids; do
  eval "t0=\$t0_$p"; t1=$(cputime "$p"); [ -n "$t1" ] || continue
  cpu=$(echo "$t0 $t1 $SECONDS_" | awk '{ printf "%.1f", ($2 - $1) / $3 }')
  mem=$(footprint -p "$p" 2>/dev/null | awk '/Footprint:/ { v = $(NF-5); u = $(NF-4); if (u == "KB") v /= 1024; else if (u == "GB") v *= 1024; printf "%.0f", v }')
  printf "  %-32s %6s %% %6s Mo\n" "$(ps -o comm= -p "$p" | sed 's|.*/||' | cut -c1-32)" "$cpu" "${mem:-?}"
  total=$(echo "$total $cpu" | awk '{ print $1 + $2 }')
done
sleep 1; grep ENVOIS "$WORK/out.txt" || true
echo "Total : $total % d'un cœur sur $SECONDS_ s ($(grep -E 'FIN|ERREUR' "$WORK/out.txt")) $(grep VISIBILITE "$WORK/out.txt" || true)"
