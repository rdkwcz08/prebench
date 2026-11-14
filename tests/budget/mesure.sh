#!/bin/sh
# tests/budget/mesure.sh — Mesure des budgets CPU et RAM de Prebench sous macOS (section 11.1).
#
# Usage : ./mesure.sh [durée en secondes, 60 par défaut]
#   Lancer Prebench, ouvrir le projet voulu (et démarrer la simulation pour la
#   mesure « en simulation »), attendre 30 s de stabilisation, puis lancer ce
#   script sans toucher à la fenêtre.
#
# Processus mesurés : l'app (prebench), ses processus WebKit (contenu,
# réseau, GPU : lancés dans la minute qui suit l'app) et le firmware. Fonctionne
# avec l'app installée (/Applications) comme avec un build local. CPU : temps CPU consommé
# pendant la mesure ÷ durée, en % d'un cœur (indépendant de la fréquence
# d'échantillonnage). RAM : empreinte physique (footprint, comme le Moniteur
# d'activité), relevée à la fin.

set -eu
DURATION="${1:-60}"

app=$(pgrep -x prebench | head -n 1 || true)
if [ -z "$app" ]; then
    echo "Prebench ne tourne pas : lancez l'app, puis relancez ce script." >&2
    exit 1
fi

# Heure de lancement d'un processus, en secondes (epoch).
started() {
    date -j -f "%a %b %d %T %Y" "$(ps -o lstart= -p "$1" | xargs)" +%s 2>/dev/null || echo 0
}
app_start=$(started "$app") # WebKit lancés après l'app : les siens
pids="$app"
for p in $(pgrep -f 'com.apple.WebKit' || true); do
    start=$(started "$p")
    # Lancés avec l'app (dans la minute) : ses processus ; un onglet Safari
    # ouvert plus tard n'est pas compté.
    if [ "$start" -ge "$((app_start - 1))" ] && [ "$start" -le "$((app_start + 60))" ]; then
        pids="$pids $p"
    fi
done
for p in $(pgrep -x firmware || true); do
    pids="$pids $p"
done

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
printf "%-45s %8s %10s\n" "Total" "$total_cpu" "$total_mem"
echo "Mesure sur ${DURATION} s ; CPU en % d'un cœur."
