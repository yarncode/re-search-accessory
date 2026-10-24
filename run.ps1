# Build the UI and the backend, then serve everything from the Rust binary.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$port = if ($env:PORT) { $env:PORT } else { "8787" }

function Show-PortConflict {
    param([string] $Port)

    Write-Host "Cong $Port dang bi chiem." -ForegroundColor Yellow
    try {
        $listeners = Get-NetTCPConnection -LocalPort ([int]$Port) -State Listen -ErrorAction Stop
        foreach ($listener in $listeners) {
            $owner = Get-Process -Id $listener.OwningProcess -ErrorAction SilentlyContinue
            Write-Host ("  PID {0}  {1}" -f $listener.OwningProcess, $owner.ProcessName) -ForegroundColor Yellow
        }
    }
    catch {
        # Get-NetTCPConnection không có trên Windows cũ
    }
    Write-Host "Tat tien trinh do roi chay lai, hoac dung cong khac:  .\run.cmd 8788" -ForegroundColor Yellow
    Write-Host ('Lenh tat nhanh: Get-NetTCPConnection -LocalPort ' + $Port + ' -State Listen | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }') -ForegroundColor DarkGray
}

$listening = $null
try {
    $listening = Get-NetTCPConnection -LocalPort ([int]$port) -State Listen -ErrorAction SilentlyContinue
}
catch {
    $listening = $null
}

if ($listening) {
    Show-PortConflict -Port $port
    exit 1
}

Write-Host "==> building UI" -ForegroundColor Cyan
if (-not (Test-Path "web/node_modules")) {
    Push-Location web; npm install; Pop-Location
}
Push-Location web; npm run build; Pop-Location

Write-Host "==> building backend (release)" -ForegroundColor Cyan
Push-Location backend; cargo build --release; Pop-Location

Write-Host "==> serving on http://127.0.0.1:$port" -ForegroundColor Green
$exe = Join-Path $PSScriptRoot "backend\target\release\re-search-accessory-backend.exe"
& $exe
