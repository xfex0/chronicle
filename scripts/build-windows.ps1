<#
  Builds the Chronicle Windows installer:  dist-windows\Chronicle_<version>_x64-setup.exe

  One-time requirements: Node.js 20 LTS, Rust (https://rustup.rs, MSVC toolchain),
  Visual Studio Build Tools with "Desktop development with C++". Python is NOT needed.

  Usage (PowerShell, from the repository root):
      powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
#>
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$App  = Join-Path $Root "apps\chronicle"
$Host_ = Join-Path $App "src-tauri"

function Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Need($cmd, $hint) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { throw "Не знайдено / not found: '$cmd'. $hint" }
}
function Run([scriptblock]$block, $what) {
    & $block
    if ($LASTEXITCODE -ne 0) { throw "Помилка / failed: $what (code $LASTEXITCODE)" }
}

Step "Tools / Інструменти"
Need node  "Install Node.js 20 LTS: https://nodejs.org"
Need npm   "npm comes with Node.js"
Need cargo "Install Rust: https://rustup.rs"

Step "Rust core tests / Тести ядра"
Push-Location (Join-Path $Root "rust")
try {
    Run { cargo test --workspace --quiet } "cargo test"
    $demo = Join-Path ([System.IO.Path]::GetTempPath()) ("chronicle-demo-" + [guid]::NewGuid())
    Run { cargo run -q -p chronicle-devtools -- demo $demo } "demo campaign"
    Run { cargo run -q -p chronicle-devtools -- check $demo } "self-test"
} finally { Pop-Location }

Push-Location $App
try {
    Step "Frontend dependencies / Залежності інтерфейсу"
    Run { npm install --no-fund --no-audit } "npm install"

    if (-not (Test-Path "$Host_\icons\icon.ico")) {
        Step "Icons / Іконки"
        Run { npm run tauri icon app-icon.png } "tauri icon"
    }

    Step "Installer (first Rust build takes 5-15 min) / Інсталятор"
    Run { npm run tauri build -- --bundles nsis } "tauri build"
} finally {
    Pop-Location
}

Step "Done / Готово"
$out = Join-Path $Root "dist-windows"
New-Item -ItemType Directory -Force $out | Out-Null
$installers = Get-ChildItem "$Host_\target\release\bundle\nsis\*.exe"
$installers | Copy-Item -Destination $out -Force
$installers | ForEach-Object { Write-Host "Installer: $(Join-Path $out $_.Name)" -ForegroundColor Green }
