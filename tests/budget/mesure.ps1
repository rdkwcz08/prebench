# tests/budget/mesure.ps1 — Mesure des budgets CPU et RAM de Prebench sous Windows (section 11.1).
#
# Usage (PowerShell) : .\mesure.ps1 [-Duree 60]
#   Lancer Prebench, ouvrir le projet voulu (et démarrer la simulation pour la
#   mesure « en simulation »), attendre 30 s de stabilisation, puis lancer ce
#   script sans toucher à la fenêtre. Si l'exécution des scripts est bloquée :
#   powershell -ExecutionPolicy Bypass -File .\mesure.ps1
#
# Processus mesurés : l'app (prebench.exe) et tous ses descendants : processus
# WebView2 (msedgewebview2.exe, lancés par l'app, contrairement à macOS où
# WebKit est partagé entre apps) et firmware (firmware-<horodatage>.exe).
# Un Edge ouvert à côté n'est pas compté : ses processus ne descendent pas de
# Prebench.
# CPU : temps CPU consommé pendant la mesure ÷ durée, en % d'un cœur.
# RAM : mémoire privée (PrivateMemorySize64), relevée à la fin.

param([int]$Duree = 60)

$app = Get-Process -Name prebench -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $app) {
    Write-Error "Prebench ne tourne pas : lancez l'app, puis relancez ce script."
    exit 1
}

# Descendants de l'app (WebView2, firmware) d'après les identifiants de parent.
$all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
$ids = @($app.Id)
do {
    $added = $false
    foreach ($p in $all) {
        if ($ids -contains $p.ParentProcessId -and -not ($ids -contains $p.ProcessId)) {
            $ids += $p.ProcessId
            $added = $true
        }
    }
} while ($added)
# Firmware lancé par le débogueur (hors arborescence de l'app) : ajouté par son
# nom. L'app, elle, lance une copie nommée run\firmware-<horodatage>.exe : elle
# fait partie des descendants ci-dessus.
Get-Process -Name 'firmware*' -ErrorAction SilentlyContinue | ForEach-Object {
    if (-not ($ids -contains $_.Id)) { $ids += $_.Id }
}

$t0 = @{}
foreach ($id in $ids) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { $t0[$id] = $p.TotalProcessorTime.TotalSeconds }
}
Start-Sleep -Seconds $Duree

$totalCpu = 0.0
$totalMem = 0.0
"{0,-8} {1,-28} {2,8} {3,10}" -f "PID", "Processus", "CPU %", "RAM (Mo)"
foreach ($id in $ids) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if (-not $p -or -not $t0.ContainsKey($id)) { continue }
    $cpu = ($p.TotalProcessorTime.TotalSeconds - $t0[$id]) / $Duree * 100
    $mem = $p.PrivateMemorySize64 / 1MB
    $totalCpu += $cpu
    $totalMem += $mem
    "{0,-8} {1,-28} {2,8:N2} {3,10:N1}" -f $id, $p.ProcessName, $cpu, $mem
}
"{0,-37} {1,8:N2} {2,10:N1}" -f "Total", $totalCpu, $totalMem
"Mesure sur $Duree s ; CPU en % d'un cœur."
