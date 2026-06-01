param(
  [string]$OutputDir = "dist",
  [string]$PackageName = "Odysseus-Windows-Portable"
)

$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$stageRoot = Join-Path $repo $OutputDir
$stage = Join-Path $stageRoot $PackageName
$zip = Join-Path $stageRoot "$PackageName.zip"
$desktopExe = Join-Path $repo "src-tauri\target\release\odysseus-desktop.exe"

if (-not (Test-Path $desktopExe)) {
  throw "Tauri desktop executable not found. Run npm run desktop:build first."
}

if (Test-Path $stage) {
  Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Path $stage | Out-Null

$excludedDirs = @(
  ".git",
  ".github",
  ".venv",
  "venv",
  "node_modules",
  "data",
  "logs",
  "dist",
  "src-tauri\target",
  "src-tauri\gen",
  ".playwright-mcp",
  "reports",
  "tasks",
  "_scratch"
)

$excludedFiles = @(
  ".env",
  "*.pyc",
  "*.pyo",
  "*.log",
  "*.db",
  "*.sqlite",
  "*.sqlite3"
)

function Test-ExcludedPath([string]$relativePath) {
  foreach ($dir in $excludedDirs) {
    if ($relativePath -eq $dir -or $relativePath.StartsWith("$dir\")) {
      return $true
    }
  }

  foreach ($pattern in $excludedFiles) {
    if ((Split-Path $relativePath -Leaf) -like $pattern) {
      return $true
    }
  }

  return $false
}

Get-ChildItem -LiteralPath $repo -Force -Recurse -File | ForEach-Object {
  $relative = $_.FullName.Substring($repo.Path.Length + 1)
  if (Test-ExcludedPath $relative) {
    return
  }

  $destination = Join-Path $stage $relative
  $destinationDir = Split-Path $destination -Parent
  if (-not (Test-Path $destinationDir)) {
    New-Item -ItemType Directory -Path $destinationDir | Out-Null
  }
  Copy-Item -LiteralPath $_.FullName -Destination $destination
}

$portableExe = Join-Path $stage "src-tauri\target\release\odysseus-desktop.exe"
New-Item -ItemType Directory -Path (Split-Path $portableExe -Parent) -Force | Out-Null
Copy-Item -LiteralPath $desktopExe -Destination $portableExe -Force

if (Test-Path $zip) {
  Remove-Item -LiteralPath $zip -Force
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($stage, $zip)

Write-Host "Created portable Windows package:"
Write-Host $zip
