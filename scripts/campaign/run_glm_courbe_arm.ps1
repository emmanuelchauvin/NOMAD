# Un bras du protocole de courbe d'echelle sur glm-5.3-flash (OpenRouter), une seed apres l'autre.
# Meme executable fige et memes reglages du moteur que run_gemini_courbe_arm.ps1 ; seuls changent
# le modele, l'adresse de l'API, la cle et les seeds.
# Usage : run_glm_courbe_arm.ps1 -Arm evolution|tirage [-From 0 -To 23]
# -From et -To bornent les rangs de seeds traites, pour repartir un bras sur plusieurs
# processus. Les runs sont independants : le decoupage ne change pas les resultats.
# Saute les seeds deja terminees : relancer le script reprend la ou il s'est arrete.
# La cle d'API est lue dans la variable d'environnement OPENROUTER_API_KEY.
# -Pilot : essai prealable hors protocole (seeds 3000 + 17 r, run arrete a 680 appels,
# dossier a part).
param(
    [Parameter(Mandatory = $true)][ValidateSet('evolution', 'tirage')][string]$Arm,
    [ValidateRange(0, 23)][int]$From = 0,
    [ValidateRange(0, 23)][int]$To = 23,
    [switch]$Pilot
)

# Evolution : 22 propositions fraiches puis ($Generations - 1) generations de 7 appels.
# 192 generations : 22 + 191 x 7 = 1359 appels. Tirage : autant de propositions fraiches.
$Generations = 192
$TirageSize = 1359

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root 'target\frozen\nomad-gemini-courbe.exe'
$seedBase = 9000
$outRoot = 'results_glm_courbe'
if ($Pilot) {
    $seedBase = 3000
    $outRoot = 'results_glm_courbe_pilote'
    $Generations = 95   # 22 + 94 x 7 = 680 appels
    $TirageSize = 680
}
$out = Join-Path $root "$outRoot\$Arm"
New-Item -ItemType Directory -Force $out | Out-Null
$log = Join-Path $out "arm_${From}_${To}.log"

if (-not $env:OPENROUTER_API_KEY) {
    # Processus lance hors d'une session interactive : la variable est lue au niveau
    # utilisateur.
    $env:OPENROUTER_API_KEY = [Environment]::GetEnvironmentVariable('OPENROUTER_API_KEY', 'User')
}
if (-not $env:OPENROUTER_API_KEY) {
    Add-Content $log "$(Get-Date -Format s) OPENROUTER_API_KEY absente"
    exit 1
}
$env:NOMAD_BP_PROFILE = 'weibull'
$env:NOMAD_FUEL_LIMIT = '400000'
$env:NOMAD_PARENT_SELECTION = 'rank'
$env:NOMAD_API_KEY_ENV = 'OPENROUTER_API_KEY'
# Raisonnement au minimum (il ne peut pas etre coupe sur ce modele) ; pas de champ `seed`,
# comme sur Gemini ; un seul hebergeur (Relace, le moins cher), sans repli sur un autre, pour
# que tous les appels des deux bras aillent a la meme version du modele.
$env:NOMAD_LLM_EXTRA_BODY = '{"reasoning":{"effort":"minimal"},"seed":null,"provider":{"order":["Relace"],"allow_fallbacks":false}}'
$env:NOMAD_LLM_TIMEOUT_SECS = '300'

if ($Arm -eq 'evolution') {
    # Reglage du protocole de confirmation (depart de 22), prolonge.
    $env:NOMAD_INIT_POP = '22'
    $env:NOMAD_STAGNATION_PATIENCE = '3'
    $env:NOMAD_INFORMED_MUTATION = '1'
    $shape = @('--pop-size', '8', '-n', "$Generations")
}
else {
    # Propositions fraiches seulement, aucune des options du moteur d'evolution.
    $env:NOMAD_INIT_POP = '0'
    $env:NOMAD_STAGNATION_PATIENCE = '0'
    $env:NOMAD_INFORMED_MUTATION = '0'
    $shape = @('--pop-size', "$TirageSize", '-n', '1')
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
            '--ollama-url', 'https://openrouter.ai/api/v1/chat/completions',
            '--model', 'z-ai/glm-5.3-flash', '--output', $tmp)
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
