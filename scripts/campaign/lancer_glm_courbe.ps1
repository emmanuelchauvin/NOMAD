# Lance les douze processus du protocole docs/PROTOCOLE_GLM_COURBE.md, detaches de la
# fenetre qui les lance : elle peut etre fermee ensuite, ainsi que VS Code.
# Usage, depuis une fenetre PowerShell ordinaire :
#   powershell -ExecutionPolicy Bypass -File G:\devs\Antigravity\NOMAD\scripts\campaign\lancer_glm_courbe.ps1
#   ... lancer_glm_courbe.ps1 -Etat     affiche l'avancement sans rien lancer
#   ... lancer_glm_courbe.ps1 -Pilot    essai prealable hors protocole : six seeds par bras
#                                       (3000 a 3085), runs arretes a 680 appels
# Protocole : 24 seeds par bras (9000 a 9391), six processus par bras, quatre seeds chacun.
# Relancer le script apres un arret reprend les seeds non terminees.
param([switch]$Etat, [switch]$Pilot)

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$arm = Join-Path $PSScriptRoot 'run_glm_courbe_arm.ps1'
if ($Pilot) {
    $out = Join-Path $root 'results_glm_courbe_pilote'
    $total = 6
    $perRun = 680
    $slices = 0..5 | ForEach-Object { , @($_, $_) }
}
else {
    $out = Join-Path $root 'results_glm_courbe'
    $total = 24
    $perRun = 1359
    $slices = 0, 4, 8, 12, 16, 20 | ForEach-Object { , @($_, ($_ + 3)) }
}

function Show-Etat {
    foreach ($a in 'evolution', 'tirage') {
        $dir = Join-Path $out $a
        $done = @(Get-ChildItem $dir -Filter "run_seed_*_$a.json" -ErrorAction SilentlyContinue).Count
        $calls = 0
        Get-ChildItem $dir -Filter 'usage_*.jsonl' -ErrorAction SilentlyContinue |
            ForEach-Object { $calls += @(Get-Content $_.FullName).Count }
        Write-Host ("{0,-9} : {1,2}/{2} runs termines, {3,5}/{4} appels" -f $a, $done, $total, $calls, ($total * $perRun))
        Get-ChildItem $dir -Filter 'arm_*.log' -ErrorAction SilentlyContinue |
            ForEach-Object { Select-String -Path $_.FullName -Pattern 'ECHEC|arret' } |
            ForEach-Object { Write-Host "   $($_.Filename) : $($_.Line)" }
    }
    $running = @(Get-Process nomad-gemini-courbe -ErrorAction SilentlyContinue).Count
    Write-Host "Processus en cours : $running"
}

if ($Etat) { Show-Etat; exit 0 }

if (-not [Environment]::GetEnvironmentVariable('OPENROUTER_API_KEY', 'User') -and -not $env:OPENROUTER_API_KEY) {
    Write-Host 'OPENROUTER_API_KEY absente : rien n''a ete lance.'
    exit 1
}
if (@(Get-Process nomad-gemini-courbe -ErrorAction SilentlyContinue).Count -gt 0) {
    Write-Host 'Des runs sont deja en cours : rien n''a ete lance.'
    Show-Etat
    exit 1
}

foreach ($a in 'evolution', 'tirage') {
    foreach ($slice in $slices) {
        $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$arm`"",
            '-Arm', $a, '-From', "$($slice[0])", '-To', "$($slice[1])")
        if ($Pilot) { $arguments += '-Pilot' }
        Start-Process powershell -WindowStyle Hidden -ArgumentList $arguments
    }
}
Start-Sleep -Seconds 20
Write-Host "Lance a $(Get-Date -Format 'HH:mm'). Cette fenetre peut etre fermee."
Show-Etat
