# Development mode: Rust backend + Vite dev server (Vite proxies /api to the backend).
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$port = if ($env:PORT) { $env:PORT } else { "8787" }

$listening = $null
try {
    $listening = Get-NetTCPConnection -LocalPort ([int]$port) -State Listen -ErrorAction SilentlyContinue
}
catch {
    $listening = $null
}

if ($listening) {
    Write-Host "Cong $port dang bi chiem." -ForegroundColor Yellow
    foreach ($listener in $listening) {
        $owner = Get-Process -Id $listener.OwningProcess -ErrorAction SilentlyContinue
        Write-Host ("  PID {0}  {1}" -f $listener.OwningProcess, $owner.ProcessName) -ForegroundColor Yellow
    }
    Write-Host "Tat tien trinh do roi chay lai, hoac dung cong khac:  .\run-dev.cmd 8788" -ForegroundColor Yellow
    exit 1
}

if (-not (Test-Path "web/node_modules")) {
    Push-Location web; npm install; Pop-Location
}

Write-Host "==> backend (port $port)" -ForegroundColor Cyan
$backend = Start-Process -FilePath "cargo" -ArgumentList "run" -WorkingDirectory (Join-Path $PSScriptRoot "backend") -PassThru -NoNewWindow

try {
    Start-Sleep -Seconds 2
    Write-Host "==> UI (port 5173) - mo http://127.0.0.1:5173" -ForegroundColor Green
    Push-Location web
    npm run dev
    Pop-Location
}
finally {
    if ($backend -and -not $backend.HasExited) {
        Stop-Process -Id $backend.Id -Force -ErrorAction SilentlyContinue
    }
}
