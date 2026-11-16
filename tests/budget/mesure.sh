#!/bin/sh
# tests/budget/mesure.sh — Mesure des budgets CPU et RAM de Prebench sous macOS (section 11.1).
#
# Usage : ./mesure.sh [durée en secondes, 60 par défaut]
#   Lancer Prebench, ouvrir le projet voulu (et démarrer la simulation pour la
#   mesure « en simulation »), attendre 30 s de stabilisation, puis lancer ce
#   script sans toucher à la fenêtre.
#
# Processus mesurés : l'app (prebench), ses processus WebKit (contenu, réseau,
# GPU) et le firmware. Les processus WebKit sont des services XPC partagés
# entre apps (Safari, Mail…), lancés par launchd : on ne garde que ceux dont
# le processus responsable (au sens de macOS) est Prebench, lu par la fonction
# système responsibility_get_pid_responsible_for_pid, via un petit programme
# compilé à la volée (cc, fourni par les outils en ligne de commande Xcode,
# prérequis de Prebench). Le firmware est un enfant de l'app
# (run/firmware-<horodatage>) ; un firmware lancé par le débogueur est
# reconnu par son nom. App lancée depuis un terminal (build de dev) : le
# responsable de ses processus WebKit est le terminal ; on garde alors ceux
# du même responsable que l'app, lancés après elle (comme app/tests-ui/wk/banc.sh).
# Si les processus WebKit ou le firmware manquent, le script le dit et marque
# le total « incomplet » au lieu d'afficher un total trompeur.
# CPU : temps CPU consommé pendant la mesure ÷ durée, en % d'un cœur
# (indépendant de la fréquence d'échantillonnage). RAM : empreinte physique
# (footprint, comme le Moniteur d'activité), relevée à la fin.

set -eu
export LC_ALL=C # nombres avec un point, quelle que soit la langue du système
DURATION="${1:-60}"

app=$(pgrep -x prebench | head -n 1 || true)
if [ -z "$app" ]; then
    echo "Prebench ne tourne pas : lancez l'app, puis relancez ce script." >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cat > "$work/resp.c" <<'C'
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
extern pid_t responsibility_get_pid_responsible_for_pid(pid_t);
int main(int argc, char **argv) {
    for (int i = 1; i < argc; ++i) {
        pid_t p = (pid_t)atoi(argv[i]);
        printf("%d %d\n", (int)p, (int)responsibility_get_pid_responsible_for_pid(p));
    }
    return 0;
}
C
if ! cc -o "$work/resp" "$work/resp.c" 2>/dev/null; then
    echo "cc introuvable : installez les outils en ligne de commande (xcode-select --install)." >&2
    exit 1
fi

# Heure de lancement d'un processus, en secondes depuis l'époque.
started() { date -j -f "%a %b %d %T %Y" "$(ps -o lstart= -p "$1" | xargs)" +%s 2>/dev/null || echo 0; }

pids="$app"
n_webkit=0
n_firmware=0
webkit=$(pgrep -f 'com.apple.WebKit' || true)
if [ -n "$webkit" ]; then
    # shellcheck disable=SC2086
    for p in $("$work/resp" $webkit | awk -v app="$app" '$2 == app { print $1 }'); do
        pids="$pids $p"
        n_webkit=$((n_webkit + 1))
    done
    if [ "$n_webkit" -eq 0 ]; then
        # App lancée depuis un terminal : même responsable que l'app, lancés après elle.
        resp_app=$("$work/resp" "$app" | awk '{ print $2 }')
        app_start=$(started "$app")
        # shellcheck disable=SC2086
        for p in $("$work/resp" $webkit | awk -v r="$resp_app" '$2 == r { print $1 }'); do
            if [ "$(started "$p")" -ge "$app_start" ]; then
                pids="$pids $p"
                n_webkit=$((n_webkit + 1))
            fi
        done
    fi
fi
for p in $(pgrep -P "$app" || true) $(pgrep '^firmware' || true); do
    case " $pids " in *" $p "*) continue ;; esac
    case "$(ps -o comm= -p "$p" | sed 's|.*/||')" in firmware*) pids="$pids $p"; n_firmware=$((n_firmware + 1)) ;; esac
done
incomplete=""
if [ "$n_webkit" -eq 0 ]; then
    echo "Avertissement : aucun processus WebKit de Prebench trouvé ; l'interface (contenu, GPU) n'est pas comptée." >&2
    incomplete=" (incomplet : sans WebKit)"
fi
if [ "$n_firmware" -eq 0 ]; then
    echo "Avertissement : aucun firmware en cours ; mesure « app ouverte, pas de simulation » seulement." >&2
fi

# Temps CPU cumulé d'un processus, en centièmes de seconde (ps : [[h:]m:]s.cc).
cputime() {
    ps -o time= -p "$1" 2>/dev/null | awk -F: '{
        n = split($0, a, ":"); s = 0;
        for (i = 1; i <= n; i++) s = s * 60 + a[i];
        printf "%d\n", s * 100 }'
}

for p in $pids; do
    eval "t0_$p=\$(cputime $p)"
done
sleep "$DURATION"

total_cpu=0
total_mem=0
printf "%-8s %-36s %8s %10s\n" "PID" "Processus" "CPU %" "RAM (Mo)"
for p in $pids; do
    eval "t0=\$t0_$p"
    t1=$(cputime "$p")
    [ -z "$t1" ] && continue
    cpu=$(echo "$t0 $t1 $DURATION" | awk '{ printf "%.2f", ($2 - $1) / $3 }')
    mem=$(footprint -p "$p" 2>/dev/null | awk '/Footprint:/ {
        v = $(NF-5); u = $(NF-4);
        if (u == "KB") v /= 1024; else if (u == "GB") v *= 1024;
        printf "%.1f", v }')
    name=$(ps -o comm= -p "$p" | sed 's|.*/||' | cut -c1-36)
    printf "%-8s %-36s %8s %10s\n" "$p" "$name" "$cpu" "${mem:-?}"
    total_cpu=$(echo "$total_cpu ${cpu:-0}" | awk '{ print $1 + $2 }')
    total_mem=$(echo "$total_mem ${mem:-0}" | awk '{ print $1 + $2 }')
done
printf "%-45s %8s %10s\n" "Total$incomplete" "$total_cpu" "$total_mem"
echo "Mesure sur ${DURATION} s ; CPU en % d'un cœur."
