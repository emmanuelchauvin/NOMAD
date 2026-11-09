# 1. Créer un dossier temporaire de staging propre
$tempDir = ".\nomad_clean_staging"
if (Test-Path $tempDir) { Remove-Item $tempDir -Recurse -Force }
New-Item -ItemType Directory -Path $tempDir | Out-Null

# 2. Copier les éléments essentiels du workspace
$itemsToCopy = @("README.md", "SPECIFICATIONS.md", "TESTS.md", "Cargo.toml", "Cargo.lock", "crates", "scripts")
foreach ($item in $itemsToCopy) {
    if (Test-Path $item) {
        Copy-Item -Path $item -Destination $tempDir -Recurse -Force
    }
}

# 3. Purger les artefacts indésirables (JSON, HTML, PDF, répertoires target et .git)
Get-ChildItem -Path $tempDir -Recurse -Include *.json, *.html, *.pdf, target, .git, .DS_Store | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

# 4. Générer l'archive nomad.zip finale
if (Test-Path "nomad.zip") { Remove-Item "nomad.zip" }
Compress-Archive -Path "$tempDir\*" -DestinationPath "nomad.zip"

# 5. Nettoyer le dossier temporaire
Remove-Item $tempDir -Recurse -Force
Write-Host "Archive nomad.zip générée avec succès !" -ForegroundColor Green

Get-ChildItem -Path crates, TESTS.md, README.md, SPECIFICATIONS.md -Recurse -Include *.rs, *.md | ForEach-Object {
    "`n===== FILE: $($_.FullName) =====`n"
    Get-Content -Path $_.FullName -Raw
} | Set-Content -Path "nomad_v2_fixed.txt" -Encoding UTF8
Write-Host "Fichier nomad_v2_fixed.txt généré avec succès !" -ForegroundColor Green
