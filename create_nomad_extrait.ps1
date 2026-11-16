# ==============================================================================
# Extrait de NOMAD pour demander l'avis d'une IA externe sur le croisement AST.
# Produit un seul fichier texte, nomad_extrait.txt, a coller ou joindre dans la conversation.
#
# Le code vient du worktree ../nomad-ast (branche ast-croisement, croisement corrige) s'il
# existe, sinon de ce depot. Aucune donnee brute de tir n'est incluse, en particulier pas
# celles du tir GLM en cours (le protocole interdit de les lire avant le depouillement).
#
# Usage : powershell -ExecutionPolicy Bypass -File create_nomad_extrait.ps1
# ==============================================================================
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$code = Join-Path (Split-Path -Parent $root) 'nomad-ast'
if (-not (Test-Path (Join-Path $code 'crates'))) { $code = $root }
$out = Join-Path $root 'nomad_extrait.txt'

# Documents, dans l'ordre de lecture. La demande d'avis vient en premier.
$docs = @(
    'docs\DEMANDE_AVIS_CROISEMENT.md',
    'README.md',
    'docs\GRID_ARCHITECTURE.md',
    'docs\RESULTATS_GEMINI_COURBE.md',
    'docs\REPRISE.md'
) | ForEach-Object { Join-Path $root $_ }

# Code utile pour juger le croisement : genome (AST, unites, greffe, controle structurel),
# controle d'ablation, moteur, archive MAP-Elites, operateur LLM, mesures hors ligne.
$sources = @(
    'crates\nomad-genome\src\*.rs',
    'crates\nomad-evaluator\src\anti_dead_code.rs',
    'crates\nomad-orchestrator\src\engine.rs',
    'crates\nomad-orchestrator\src\map_elites.rs',
    'crates\nomad-operator\src\http_llm.rs',
    'crates\nomad-cli\examples\*.rs'
) | ForEach-Object { Get-ChildItem (Join-Path $code $_) -ErrorAction SilentlyContinue } |
    ForEach-Object { $_.FullName }

$files = @($docs) + @($sources) | Where-Object { Test-Path $_ }
$branch = git -C $code rev-parse --abbrev-ref HEAD
$commit = git -C $code rev-parse --short HEAD

function Get-Rel($f) {
    if ($f.StartsWith($code + '\crates')) { $f.Substring($code.Length + 1) } else { $f.Substring($root.Length + 1) }
}

$parts = @("NOMAD - extrait pour avis externe. Code : branche $branch, commit $commit.",
    "Fichiers inclus :")
$parts += $files | ForEach-Object { '  ' + (Get-Rel $_) }
foreach ($f in $files) {
    $rel = Get-Rel $f
    $parts += "`n================================================================================"
    $parts += "===== FICHIER : $rel ====="
    $parts += "================================================================================`n"
    $parts += Get-Content -Path $f -Raw -Encoding UTF8
}
$parts | Set-Content -Path $out -Encoding UTF8

$size = (Get-Item $out).Length
Write-Host ("nomad_extrait.txt : {0} fichiers, {1:N0} Ko, environ {2:N0} tokens (code : $branch $commit)" -f $files.Count, ($size / 1KB), ($size / 3.5))
