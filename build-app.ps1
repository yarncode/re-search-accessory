# Build the desktop app: bundled UI + Rust backend in one window.
# Output: .\re-search-accessory.exe (+ WebView2Loader.dll beside it)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

Write-Host "==> building UI" -ForegroundColor Cyan
if (-not (Test-Path "web/node_modules")) {
    Push-Location web; npm install; Pop-Location
}
Push-Location web; npm run build; Pop-Location

Write-Host "==> building desktop app (release)" -ForegroundColor Cyan
Push-Location backend
cargo build --release --bin re-search-accessory-app
Pop-Location

$exe = "backend\target\release\re-search-accessory-app.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Khong tim thay $exe" -ForegroundColor Red
    exit 1
}

Copy-Item $exe ".\re-search-accessory.exe" -Force

# WebView2 loader must sit next to the exe at runtime.
$loader = Get-ChildItem "backend\target\release\build\webview2-com-sys-*\out\x64\WebView2Loader.dll" -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $loader) {
    $loader = Get-Item "backend\target\release\WebView2Loader.dll" -ErrorAction SilentlyContinue
}
if ($loader) {
    Copy-Item $loader.FullName ".\WebView2Loader.dll" -Force
}
else {
    Write-Host "Canh bao: khong tim thay WebView2Loader.dll" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Xong. Bam doi vao:  .\re-search-accessory.exe" -ForegroundColor Green
Write-Host "App can WebView2 runtime (Windows 10/11 co san qua Microsoft Edge)." -ForegroundColor DarkGray
