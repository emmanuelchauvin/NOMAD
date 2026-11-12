# Temoin "tirage sans evolution" : voir docs/PROTOCOLE_WEIBULL_TEMOIN.md.
# Saute les seeds deja terminees : relancer le script reprend la ou il s'est arrete.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root 'target\frozen\nomad-campaign-weibull-v2.exe'
$out = Join-Path $root 'results_weibull_temoin'
New-Item -ItemType Directory -Force $out | Out-Null
$env:NOMAD_BP_PROFILE = 'weibull'
$env:NOMAD_FUEL_LIMIT = '400000'
$log = Join-Path $out 'temoin.log'

foreach ($r in 0..11) {
    $seed = 2000 + 17 * $r
    $report = Join-Path $out "run_seed_${seed}_tirage.json"
    if (Test-Path $report) {
        Add-Content $log "$(Get-Date -Format s) seed $seed deja terminee"
        continue
    }
    Add-Content $log "$(Get-Date -Format s) seed $seed debut"
    $tmp = "$report.tmp"
    $p = Start-Process -FilePath $exe -WorkingDirectory $root -NoNewWindow -Wait -PassThru `
        -ArgumentList 'run', '-G', 'D', '--seed', $seed, '--pop-size', '85', '-n', '1',
        '--operator', 'ollama', '--model', 'gpt-oss:20b', '--output', $tmp `
        -RedirectStandardOutput (Join-Path $out "run_seed_${seed}.out.log") `
        -RedirectStandardError (Join-Path $out "run_seed_${seed}.err.log")
    if ($p.ExitCode -ne 0 -or -not (Test-Path $tmp)) {
        Add-Content $log "$(Get-Date -Format s) seed $seed ECHEC (code $($p.ExitCode))"
        exit 1
    }
    Move-Item $tmp $report
    Add-Content $log "$(Get-Date -Format s) seed $seed fin"
}
Add-Content $log "$(Get-Date -Format s) temoin termine"
