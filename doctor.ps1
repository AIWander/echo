# echo MCP Server — Diagnostic Script
# Checks binary presence, Ollama reachability, and available models.

param(
    [string]$BinaryPath = "C:\CPC\servers\echo.exe"
)

$ErrorActionPreference = "Continue"
$passed = 0
$failed = 0

Write-Host "`n=== echo Doctor ===" -ForegroundColor Cyan
Write-Host ""

# Check 1: Binary exists
Write-Host "[1/3] Checking echo binary..." -ForegroundColor Yellow
if (Test-Path $BinaryPath) {
    $info = Get-Item $BinaryPath
    $sizeMB = [math]::Round($info.Length / 1MB, 1)
    Write-Host "  PASS: Found $BinaryPath ($sizeMB MB)" -ForegroundColor Green
    $passed++
} else {
    Write-Host "  FAIL: Binary not found at $BinaryPath" -ForegroundColor Red
    Write-Host "  Fix: Download from https://github.com/AIWander/echo/releases" -ForegroundColor Gray
    $failed++
}

# Check 2: Ollama reachable
Write-Host "[2/3] Checking Ollama at localhost:11434..." -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "http://localhost:11434/api/tags" -Method Get -TimeoutSec 5
    Write-Host "  PASS: Ollama is reachable" -ForegroundColor Green
    $passed++
} catch {
    Write-Host "  FAIL: Cannot reach Ollama at localhost:11434" -ForegroundColor Red
    Write-Host "  Fix: Start Ollama with: ollama serve" -ForegroundColor Gray
    $failed++
    $response = $null
}

# Check 3: List available models
Write-Host "[3/3] Listing available Ollama models..." -ForegroundColor Yellow
if ($response -and $response.models) {
    $models = $response.models
    if ($models.Count -gt 0) {
        Write-Host "  PASS: $($models.Count) model(s) available:" -ForegroundColor Green
        foreach ($m in $models) {
            $sizeMB = [math]::Round($m.size / 1MB, 0)
            Write-Host "    - $($m.name) ($sizeMB MB)" -ForegroundColor White
        }
        $passed++
    } else {
        Write-Host "  FAIL: No models pulled" -ForegroundColor Red
        Write-Host "  Fix: Pull a model with: ollama pull nomic-embed-text" -ForegroundColor Gray
        $failed++
    }
} elseif ($response) {
    Write-Host "  WARN: Ollama responded but no models found" -ForegroundColor Yellow
    Write-Host "  Fix: Pull a model with: ollama pull nomic-embed-text" -ForegroundColor Gray
    $failed++
} else {
    Write-Host "  SKIP: Cannot list models (Ollama not reachable)" -ForegroundColor Gray
}

# Summary
Write-Host ""
Write-Host "=== Results ===" -ForegroundColor Cyan
Write-Host "  Passed: $passed" -ForegroundColor Green
Write-Host "  Failed: $failed" -ForegroundColor $(if ($failed -gt 0) { "Red" } else { "Green" })

if ($failed -eq 0) {
    Write-Host "`necho is ready to use." -ForegroundColor Green
} else {
    Write-Host "`nFix the issues above before using echo." -ForegroundColor Yellow
}
