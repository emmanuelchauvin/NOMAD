# Lance les processus du protocole docs/PROTOCOLE_GLM_H3.md, detaches de la fenetre
# qui les lance : elle peut etre fermee ensuite, ainsi que VS Code.
# Usage, depuis une fenetre PowerShell ordinaire :
#   powershell -ExecutionPolicy Bypass -File C:\Devs\nomad\scripts\campaign\lancer_glm_h3.ps1
#   ... lancer_glm_h3.ps1 -Etat     affiche l'avancement sans rien lancer
#   ... lancer_glm_h3.ps1 -Essai    essai de fumee hors protocole : une seed par bras (3153),
#                                    29 appels, pour verifier la cle et le reseau
# Protocole : 24 seeds par bras (10400 a 10791). Evolution : un processus par seed (le bras
# le plus long) ; tirage : douze processus de deux seeds.
# Relancer le script apres un arret reprend les seeds non terminees.
param([switch]$Etat, [switch]$Essai)

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$arm = Join-Path $PSScriptRoot 'run_glm_h3_arm.ps1'
if ($Essai) {
    $out = Join-Path $root 'results_glm_h3_essai'
    $total = 1
    $perRun = 29
    $slices = @{ evolution = (, @(0, 0)); tirage = (, @(0, 0)) }
}
else {
    $out = Join-Path $root 'results_glm_h3'
    $total = 24
    $perRun = 1359
    $slices = @{
        evolution = (0..23 | ForEach-Object { , @($_, $_) })
        tirage    = (0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22 | ForEach-Object { , @($_, ($_ + 1)) })
    }
}

function Show-Etat {
    foreach ($a in 'evolution', 'tirage') {
        $dir = Join-Path $out $a
        $done = @(Get-ChildItem $dir -Filter "run_seed_*_$a.json" -ErrorAction SilentlyContinue).Count
        $calls = 0
        Get-ChildItem $dir -Filter 'usage_*.jsonl' -ErrorAction SilentlyContinue |
            ForEach-Object { $calls += @(Get-Content $_.FullName).Count }
        Write-Host ("{0,-9} : {1,2}/{2} runs termines, {3,6}/{4} appels" -f $a, $done, $total, $calls, ($total * $perRun))
        Get-ChildItem $dir -Filter 'arm_*.log' -ErrorAction SilentlyContinue |
            ForEach-Object { Select-String -Path $_.FullName -Pattern 'ECHEC|arret' } |
            ForEach-Object { Write-Host "   $($_.Filename) : $($_.Line)" }
    }
    $running = @(Get-Process nomad-h2-elagage -ErrorAction SilentlyContinue).Count
    Write-Host "Processus en cours : $running"
}

if ($Etat) { Show-Etat; exit 0 }

if (-not [Environment]::GetEnvironmentVariable('OPENROUTER_API_KEY', 'User') -and -not $env:OPENROUTER_API_KEY) {
    Write-Host 'OPENROUTER_API_KEY absente : rien n''a ete lance.'
    exit 1
}
if (@(Get-Process nomad-h2-elagage -ErrorAction SilentlyContinue).Count -gt 0) {
    Write-Host 'Des runs sont deja en cours : rien n''a ete lance.'
    Show-Etat
    exit 1
}

foreach ($a in 'evolution', 'tirage') {
    foreach ($slice in $slices[$a]) {
        $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$arm`"",
            '-Arm', $a, '-From', "$($slice[0])", '-To', "$($slice[1])")
        if ($Essai) { $arguments += '-Essai' }
        Start-Process powershell -WindowStyle Hidden -ArgumentList $arguments
    }
}
Start-Sleep -Seconds 20
Write-Host "Lance a $(Get-Date -Format 'HH:mm'). Cette fenetre peut etre fermee."
Show-Etat
