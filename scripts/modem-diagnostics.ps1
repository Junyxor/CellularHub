$ErrorActionPreference = 'Continue'

$outFile = Join-Path $PWD 'cellularhub-diagnostics.txt'
$lines = New-Object System.Collections.Generic.List[string]

function Add-Line([string]$Text = '') {
    $lines.Add($Text)
    Write-Host $Text
}

function Section([string]$Title) {
    Add-Line ''
    Add-Line "=== $Title ==="
}

Add-Line 'CellularHub passive Windows modem diagnostics'
Add-Line "Generated: $(Get-Date -Format o)"
Add-Line "Computer: $env:COMPUTERNAME"

Section 'Windows'
try {
    $os = Get-CimInstance Win32_OperatingSystem
    Add-Line "Caption: $($os.Caption)"
    Add-Line "Version: $($os.Version)"
    Add-Line "Build: $($os.BuildNumber)"
} catch {
    Add-Line "Failed to query OS: $($_.Exception.Message)"
}

Section 'Developer tools'
foreach ($tool in @('git', 'node', 'npm', 'rustc', 'cargo')) {
    try {
        $command = Get-Command $tool -ErrorAction Stop
        $version = & $tool --version 2>&1 | Select-Object -First 1
        Add-Line "$tool: $version"
    } catch {
        Add-Line "$tool: not found"
    }
}

Section 'LPA URI handler'
try {
    $result = & reg.exe query 'HKCR\lpa' 2>&1
    if ($LASTEXITCODE -eq 0) {
        Add-Line 'HKCR\lpa: present'
    } else {
        Add-Line 'HKCR\lpa: not present'
    }
} catch {
    Add-Line "LPA check failed: $($_.Exception.Message)"
}

Section 'Serial ports'
try {
    $ports = Get-CimInstance Win32_SerialPort | Sort-Object DeviceID
    if (-not $ports) {
        Add-Line 'No Win32_SerialPort entries found.'
    }
    foreach ($port in $ports) {
        Add-Line "$($port.DeviceID) | $($port.Name) | Status=$($port.Status)"
    }
} catch {
    Add-Line "Serial-port query failed: $($_.Exception.Message)"
}

Section 'Possible WWAN / modem PnP devices'
try {
    $devices = Get-CimInstance Win32_PnPEntity |
        Where-Object {
            $_.Name -match 'WWAN|Mobile Broadband|MBIM|Cellular|LTE|5G|Modem' -or
            $_.Description -match 'WWAN|Mobile Broadband|MBIM|Cellular|LTE|5G|Modem'
        } |
        Sort-Object Name
    if (-not $devices) {
        Add-Line 'No obvious WWAN/modem PnP devices found by name/description.'
    }
    foreach ($device in $devices) {
        Add-Line "$($device.Name) | $($device.Description) | Status=$($device.Status)"
    }
} catch {
    Add-Line "PnP query failed: $($_.Exception.Message)"
}

Section 'Network adapters likely related to cellular'
try {
    $adapters = Get-NetAdapter -ErrorAction Stop |
        Where-Object {
            $_.Name -match 'Cellular|Mobile|WWAN' -or
            $_.InterfaceDescription -match 'Cellular|Mobile|WWAN|MBIM|LTE|5G'
        } |
        Sort-Object Name
    if (-not $adapters) {
        Add-Line 'No obvious cellular network adapters found.'
    }
    foreach ($adapter in $adapters) {
        Add-Line "$($adapter.Name) | $($adapter.InterfaceDescription) | Status=$($adapter.Status)"
    }
} catch {
    Add-Line "Network-adapter query failed: $($_.Exception.Message)"
}

Section 'Notes'
Add-Line 'This script is passive. It does not open a COM port, send AT commands, read SMS contents, or expose saved SMS data.'
Add-Line 'If sharing this file publicly, review it first for machine/device names you consider sensitive.'

$lines | Set-Content -Path $outFile -Encoding UTF8
Write-Host "`nSaved: $outFile" -ForegroundColor Green
