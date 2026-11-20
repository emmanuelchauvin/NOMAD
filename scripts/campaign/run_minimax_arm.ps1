# Un bras du protocole docs/PROTOCOLE_MINIMAX_TIRAGE_EVOLUTION.md, une seed apres l'autre.
# Usage : run_minimax_arm.ps1 -Arm tirage|evolution
# Saute les seeds deja terminees : relancer le script reprend la ou il s'est arrete.
# La cle d'API est lue dans la variable d'environnement MINIMAX_API_KEY.
param([Parameter(Mandatory = $true)][ValidateSet('tirage', 'evolution')][string]$Arm)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root 'target\frozen\nomad-minimax-v1.exe'
$out = Join-Path $root "results_minimax_v1\$Arm"
New-Item -ItemType Directory -Force $out | Out-Null
$log = Join-Path $out 'arm.log'

if (-not $env:MINIMAX_API_KEY) {
    Add-Content $log "$(Get-Date -Format s) MINIMAX_API_KEY absente"
    exit 1
}
$env:NOMAD_BP_PROFILE = 'weibull'
$env:NOMAD_FUEL_LIMIT = '400000'
$env:NOMAD_PARENT_SELECTION = 'rank'
$env:NOMAD_API_KEY_ENV = 'MINIMAX_API_KEY'
$env:NOMAD_LLM_EXTRA_BODY = '{"thinking":{"type":"disabled"}}'
$env:NOMAD_LLM_TIMEOUT_SECS = '300'

if ($Arm -eq 'tirage') { $shape = @('--pop-size', '85', '-n', '1') }
else { $shape = @('--pop-size', '8', '-n', '12') }

foreach ($r in 0..11) {
    $seed = 4000 + 17 * $r
    $report = Join-Path $out "run_seed_${seed}_$Arm.json"
    if (Test-Path $report) {
        Add-Content $log "$(Get-Date -Format s) seed $seed deja terminee"
        continue
    }
    $tmp = "$report.tmp"
    $done = $false
    foreach ($attempt in 1..2) {
        Add-Content $log "$(Get-Date -Format s) seed $seed debut (essai $attempt)"
        $env:NOMAD_USAGE_LOG = Join-Path $out "usage_seed_${seed}_essai${attempt}.jsonl"
        $arguments = @('run', '-G', 'D', '--seed', "$seed") + $shape + @(
            '--operator', 'ollama',
            '--ollama-url', 'https://api.minimax.io/v1/chat/completions',
            '--model', 'MiniMax-M3', '--output', $tmp)
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
        Start-Sleep -Seconds 60
    }
    if (-not $done) {
        Add-Content $log "$(Get-Date -Format s) arret : seed $seed en echec deux fois"
        exit 1
    }
    Move-Item $tmp $report
    Add-Content $log "$(Get-Date -Format s) seed $seed fin"
}
Add-Content $log "$(Get-Date -Format s) bras $Arm termine"
