# ==============================================================================
# Script de packaging NOMAD V2 pour audit et revue par IA externe
# Inclut le code source, la documentation, et les résultats complets du tir (30 seeds)
# ==============================================================================

# 1. Créer un dossier temporaire de staging propre
$tempDir = ".\nomad_clean_staging"
if (Test-Path $tempDir) { Remove-Item $tempDir -Recurse -Force }
New-Item -ItemType Directory -Path $tempDir | Out-Null

# 2. Copier les éléments essentiels du workspace (Code, Docs, Scripts, Résultats)
$itemsToCopy = @(
    "README.md",
    "SPECIFICATIONS.md",
    "TESTS.md",
    "RAPPORT_CAMPAGNE_30_SEEDS.md",
    "RAPPORT_SCHEDULING_MULTI_RESSOURCES.md",
    "Cargo.toml",
    "Cargo.lock",
    "crates",
    "scripts",
    "campaign_results",
    "scheduling_results",
    "results_scheduling_validation",
    "results_scheduling_ollama"
)

foreach ($item in $itemsToCopy) {
    if (Test-Path $item) {
        Copy-Item -Path $item -Destination $tempDir -Recurse -Force
    }
}

# 3. Purger uniquement les artefacts superflus (builds, caches, VCS)
Get-ChildItem -Path $tempDir -Recurse -Include target, .git, .DS_Store, __pycache__, *.pyc, *.html, *.pdf | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

# 4. Générer l'archive nomad.zip finale (contenant code + docs + campaign_results complet)
if (Test-Path "nomad.zip") { Remove-Item "nomad.zip" }
Compress-Archive -Path "$tempDir\*" -DestinationPath "nomad.zip" -CompressionLevel Optimal

# 5. Nettoyer le dossier temporaire
Remove-Item $tempDir -Recurse -Force
Write-Host "Archive nomad.zip générée avec succès ! (Code + Données du tir inclus)" -ForegroundColor Green

# 6. Générer nomad_v2_fixed.txt (fichier tout-en-un optimisé pour le prompt d'une IA)
# Concatène le code Rust, la documentation, le rapport de campagne et les statistiques JSON/CSV
$filesForText = @()
$filesForText += Get-ChildItem -Path crates -Recurse -Filter *.rs
$filesForText += Get-Item -Path "README.md", "SPECIFICATIONS.md", "TESTS.md", "RAPPORT_CAMPAGNE_30_SEEDS.md", "RAPPORT_SCHEDULING_MULTI_RESSOURCES.md" -ErrorAction SilentlyContinue
$filesForText += Get-Item -Path "campaign_results\stats.json", "campaign_results\stats.csv" -ErrorAction SilentlyContinue
$filesForText += Get-Item -Path "results_scheduling_validation\stats.json", "results_scheduling_ollama\stats.json" -ErrorAction SilentlyContinue
$filesForText += Get-Item -Path "scheduling_results\scheduling_runs3.json", "scheduling_results\scheduling_rebirth.json", "scheduling_results\scheduling_step4_oodprobe.json" -ErrorAction SilentlyContinue

$filesForText | ForEach-Object {
    "`n================================================================================"
    "===== FILE: $($_.FullName) ====="
    "================================================================================`n"
    Get-Content -Path $_.FullName -Raw
} | Set-Content -Path "nomad_v2_fixed.txt" -Encoding UTF8

Write-Host "Fichier nomad_v2_fixed.txt généré avec succès ! (Rapport + stats.json/csv inclus)" -ForegroundColor Green
