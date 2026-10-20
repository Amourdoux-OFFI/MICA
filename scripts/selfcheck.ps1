param([switch]$NoGui)
# MICA self-check: environment + GUI + single instance + startup log
# -NoGui: skip window launch (use while gaming / headless)
$ErrorActionPreference = 'Continue'
$root = 'D:\Desktop\DSH\MICA'
$data = Join-Path $root '.data'
$out  = Join-Path $data 'selfcheck'
$exe  = Join-Path $root 'src-tauri\target\debug\mica-cli.exe'
$rel  = Join-Path $root 'release\MICA.exe'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$rows = @()

function Row([string]$name, [bool]$ok, [string]$detail) {
    $script:rows += [pscustomobject]@{ Name = $name; Pass = $ok; Detail = $detail }
    $flag = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("[{0}] {1} :: {2}" -f $flag, $name, $detail)
}

# 1. doctor
$doc = & $exe --data $data doctor 2>$null | Out-String
$doc | Set-Content (Join-Path $out 'doctor.json') -Encoding utf8
try {
    $d = $doc | ConvertFrom-Json
    Row 'doctor' ([bool]$d.ok) ("checks={0} java={1} webview2={2}" -f $d.diagnostics.checks.Count, $d.diagnostics.javaCount, $d.diagnostics.webview2)
} catch {
    Row 'doctor' $false 'doctor output not parseable'
}

# 2. GUI startup + log
$logDirs = @(
    (Join-Path $env:APPDATA 'MICA\logs'),
    (Join-Path (Split-Path $rel) 'MICA-Data\logs'),
    (Join-Path $env:LOCALAPPDATA 'MICA\logs')
)

if ((Test-Path $rel) -and (-not $NoGui)) {
    & schtasks.exe /create /tn MicaSelfCheck /tr $rel /sc once /st 23:59 /f | Out-Null
    & schtasks.exe /run /tn MicaSelfCheck | Out-Null
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $title = ''
    while ($sw.Elapsed.TotalSeconds -lt 25) {
        Start-Sleep -Milliseconds 200
        $p = Get-Process mica -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($p -and $p.MainWindowTitle) { $title = $p.MainWindowTitle; break }
    }
    $sw.Stop()
    Row 'gui-window' ($title -eq 'MICA') ("title={0} first-paint={1:N1}s" -f $title, $sw.Elapsed.TotalSeconds)

    & schtasks.exe /run /tn MicaSelfCheck | Out-Null
    Start-Sleep -Seconds 4
    $count = @(Get-Process mica -ErrorAction SilentlyContinue).Count
    Row 'single-instance' ($count -eq 1) ("processes={0}" -f $count)
    & schtasks.exe /delete /tn MicaSelfCheck /f | Out-Null
    Start-Sleep -Seconds 1
    Get-Process mica -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

    $logPath = $null
    foreach ($d in $logDirs) {
        $p = Join-Path $d 'mica.log'
        if (Test-Path $p) { $logPath = $p; break }
    }
    if ($logPath) {
        $content = Get-Content $logPath -Raw -ErrorAction SilentlyContinue
        $hasSetup = $content -match 'setup'
        Row 'startup-log' $hasSetup $logPath
        Copy-Item $logPath (Join-Path $out 'mica.log') -Force -ErrorAction SilentlyContinue
    } else {
        Row 'startup-log' $false 'mica.log not found'
    }
} else {
    if ($NoGui) { Row 'gui-window' $true 'skipped (-NoGui)' } else { Row 'gui-window' $false "missing $rel" }
}

$rows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $out 'summary.json') -Encoding utf8
$pass = @($rows | Where-Object { $_.Pass }).Count
Write-Host ("SELFCHECK {0}/{1} passed -> {2}" -f $pass, $rows.Count, $out)
