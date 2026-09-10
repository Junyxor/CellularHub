$ErrorActionPreference = 'Stop'

function Step([string]$Name, [scriptblock]$Action) {
    Write-Host "`n==> $Name" -ForegroundColor Cyan
    & $Action
}

Write-Host 'CellularHub local preflight' -ForegroundColor Green
Write-Host "Working directory: $PWD"

Step 'Tool versions' {
    git --version
    node --version
    npm --version
    rustc --version
    cargo --version
}

Step 'Install frontend dependencies' {
    npm install
}

Step 'TypeScript + Vite build' {
    npm run build
}

Step 'Rust tests' {
    cargo test --manifest-path .\src-tauri\Cargo.toml
}

Step 'Rust check' {
    cargo check --manifest-path .\src-tauri\Cargo.toml
}

Write-Host "`nPreflight passed. Next command:" -ForegroundColor Green
Write-Host 'npm run tauri dev'
