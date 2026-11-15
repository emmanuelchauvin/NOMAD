# Lance les douze processus du protocole docs/PROTOCOLE_GEMINI_COURBE.md, detaches de la
# fenetre qui les lance : elle peut etre fermee ensuite, ainsi que VS Code.
# Usage, depuis une fenetre PowerShell ordinaire :
#   powershell -ExecutionPolicy Bypass -File G:\devs\Antigravity\NOMAD\scripts\campaign\lancer_gemini_courbe.ps1
#   ... lancer_gemini_courbe.ps1 -Etat     affiche l'avancement sans rien lancer
# Relancer le script apres un arret reprend les seeds non terminees.
#   ... lancer_gemini_courbe.ps1 -Extension   lance l'extension prevue par le protocole :
#       les 12 seeds supplementaires (rangs 12 a 23, seeds 8204 a 8391), a l'identique.
param([switch]$Etat, [switch]$Extension)

$total = 12
$ranks = 0, 2, 4, 6, 8, 10
if ($Extension) {
    $total = 24
    $ranks = 12, 14, 16, 18, 20, 22
}

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$arm = Join-Path $PSScriptRoot 'run_gemini_courbe_arm.ps1'
$out = Join-Path $root 'results_gemini_courbe'

function Show-Etat {
    foreach ($a in 'evolution', 'tirage') {
        $dir = Join-Path $out $a
        $done = @(Get-ChildItem $dir -Filter "run_seed_*_$a.json" -ErrorAction SilentlyContinue).Count
        $calls = 0
        Get-ChildItem $dir -Filter 'usage_*.jsonl' -ErrorAction SilentlyContinue |
            ForEach-Object { $calls += @(Get-Content $_.FullName).Count }
        Write-Host ("{0,-9} : {1,2}/{2} runs termines, {3,5}/{4} appels" -f $a, $done, $total, $calls, ($total * 1359))
        Get-ChildItem $dir -Filter 'arm_*.log' -ErrorAction SilentlyContinue |
            ForEach-Object { Select-String -Path $_.FullName -Pattern 'ECHEC|arret' } |
            ForEach-Object { Write-Host "   $($_.Filename) : $($_.Line)" }
    }
    $running = @(Get-Process nomad-gemini-courbe -ErrorAction SilentlyContinue).Count
    Write-Host "Processus en cours : $running"
}

if ($Etat) { Show-Etat; exit 0 }

if (-not [Environment]::GetEnvironmentVariable('GEMINI_API_KEY', 'User') -and -not $env:GEMINI_API_KEY) {
    Write-Host 'GEMINI_API_KEY absente : rien n''a ete lance.'
    exit 1
}
if (@(Get-Process nomad-gemini-courbe -ErrorAction SilentlyContinue).Count -gt 0) {
    Write-Host 'Des runs sont deja en cours : rien n''a ete lance.'
    Show-Etat
    exit 1
}

foreach ($a in 'evolution', 'tirage') {
    foreach ($from in $ranks) {
        Start-Process powershell -WindowStyle Hidden -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$arm`"",
            '-Arm', $a, '-From', "$from", '-To', "$($from + 1)")
    }
}
Start-Sleep -Seconds 20
Write-Host "Lance a $(Get-Date -Format 'HH:mm'). Cette fenetre peut etre fermee."
Show-Etat
