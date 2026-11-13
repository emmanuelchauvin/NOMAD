# Un bras du protocole docs/PROTOCOLE_GEMINI_ECHELLE.md, une seed apres l'autre.
# Usage : run_gemini_scale_arm.ps1 -Arm evolution340|tirage340|tirageDepense [-From 0 -To 23]
# -From et -To bornent les rangs de seeds traites, pour repartir un bras sur plusieurs
# processus. Les runs sont independants : le decoupage ne change pas les resultats.
# Saute les seeds deja terminees : relancer le script reprend la ou il s'est arrete.
# La cle d'API est lue dans la variable d'environnement GEMINI_API_KEY.
# -Pilot : essai prealable hors protocole (seeds 3000, 3017, ..., dossier a part).
param(
    [Parameter(Mandatory = $true)][ValidateSet('evolution340', 'tirage340', 'tirageDepense')][string]$Arm,
    [ValidateRange(0, 23)][int]$From = 0,
    [ValidateRange(0, 23)][int]$To = 23,
    [switch]$Pilot
)

# Nombre de propositions du bras a depense egale, fixe par le protocole d'apres l'essai
# prealable.
$TirageDepense = 455

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root 'target\frozen\nomad-gemini-echelle.exe'
$seedBase = 7000
$outRoot = 'results_gemini_echelle'
if ($Pilot) {
    $seedBase = 3000
    $outRoot = 'results_gemini_echelle_pilote'
}
$out = Join-Path $root "$outRoot\$Arm"
New-Item -ItemType Directory -Force $out | Out-Null
$log = Join-Path $out "arm_${From}_${To}.log"

if (-not $env:GEMINI_API_KEY) {
    Add-Content $log "$(Get-Date -Format s) GEMINI_API_KEY absente"
    exit 1
}
$env:NOMAD_BP_PROFILE = 'weibull'
$env:NOMAD_FUEL_LIMIT = '400000'
$env:NOMAD_PARENT_SELECTION = 'rank'
$env:NOMAD_API_KEY_ENV = 'GEMINI_API_KEY'
# Raisonnement au minimum (il ne peut pas etre coupe) ; l'API refuse le champ `seed`.
$env:NOMAD_LLM_EXTRA_BODY = '{"reasoning_effort":"minimal","seed":null}'
$env:NOMAD_LLM_TIMEOUT_SECS = '300'

if ($Arm -eq 'evolution340') {
    # 88 propositions fraiches puis 36 generations de 7 appels : 88 + 252 = 340 appels.
    # C'est le bras evolution du protocole de confirmation, les deux parts multipliees par 4.
    $env:NOMAD_INIT_POP = '88'
    $env:NOMAD_STAGNATION_PATIENCE = '3'
    $env:NOMAD_INFORMED_MUTATION = '1'
    $shape = @('--pop-size', '8', '-n', '37')
}
else {
    # Propositions fraiches seulement, aucune des options du moteur d'evolution.
    $env:NOMAD_INIT_POP = '0'
    $env:NOMAD_STAGNATION_PATIENCE = '0'
    $env:NOMAD_INFORMED_MUTATION = '0'
    if ($Arm -eq 'tirage340') { $shape = @('--pop-size', '340', '-n', '1') }
    else {
        if ($TirageDepense -le 0) {
            Add-Content $log "$(Get-Date -Format s) TirageDepense non fixe"
            exit 1
        }
        $shape = @('--pop-size', "$TirageDepense", '-n', '1')
    }
}

foreach ($r in $From..$To) {
    $seed = $seedBase + 17 * $r
    $report = Join-Path $out "run_seed_${seed}_$Arm.json"
    if (Test-Path $report) {
        Add-Content $log "$(Get-Date -Format s) seed $seed deja terminee"
        continue
    }
    $tmp = "$report.tmp"
    $done = $false
    foreach ($attempt in 1..3) {
        Add-Content $log "$(Get-Date -Format s) seed $seed debut (essai $attempt)"
        $env:NOMAD_USAGE_LOG = Join-Path $out "usage_seed_${seed}_essai${attempt}.jsonl"
        $arguments = @('run', '-G', 'D', '--seed', "$seed") + $shape + @(
            '--operator', 'ollama',
            '--ollama-url', 'https://generativelanguage.googleapis.com/v1beta/openai/chat/completions',
            '--model', 'gemini-3.1-flash-lite', '--output', $tmp)
        $p = Start-Process -FilePath $exe -WorkingDirectory $root -NoNewWindow -Wait -PassThru `
            -ArgumentList $arguments `
            -RedirectStandardOutput (Join-Path $out "run_seed_${seed}_essai${attempt}.out.log") `
            -RedirectStandardError (Join-Path $out "run_seed_${seed}_essai${attempt}.err.log")
        if ($p.ExitCode -eq 0 -and (Test-Path $tmp)) {
            $done = $true
            break
        }
        Add-Content $log "$(Get-Date -Format s) seed $seed ECHEC essai $attempt (code $($p.ExitCode))"
        Remove-Item $tmp -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 120
    }
    if (-not $done) {
        Add-Content $log "$(Get-Date -Format s) arret : seed $seed en echec trois fois"
        exit 1
    }
    Move-Item $tmp $report
    Add-Content $log "$(Get-Date -Format s) seed $seed fin"
}
Add-Content $log "$(Get-Date -Format s) bras $Arm rangs $From a $To termine"
