# Keep one packaging implementation for macOS, Linux and Windows.
# Outputs and versioning are documented in pack.py and docs/native-browser-extension.md.
$ErrorActionPreference = "Stop"
$python = Get-Command python3 -ErrorAction SilentlyContinue
if (-not $python) { $python = Get-Command python -ErrorAction Stop }
& $python.Source (Join-Path $PSScriptRoot "pack.py")
if ($LASTEXITCODE -ne 0) { throw "Extension packaging failed ($LASTEXITCODE)" }
