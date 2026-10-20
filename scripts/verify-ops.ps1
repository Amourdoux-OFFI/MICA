$ErrorActionPreference = 'Continue'
$root = 'D:\Desktop\DSH\MICA'
$acc  = Join-Path $root '.data\acceptance'
$sc   = Join-Path $root '.data\selfcheck'
$rows = @()
function Row([string]$id, [string]$name, [bool]$ok, [string]$detail) {
    $script:rows += [pscustomobject]@{ Id = $id; Name = $name; Pass = $ok; Evidence = $detail }
    $flag = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("[{0}] {1} {2} :: {3}" -f $flag, $id, $name, $detail)
}

$summary = [IO.File]::ReadAllText((Join-Path $sc 'summary.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
$doctor = [IO.File]::ReadAllText((Join-Path $sc 'doctor.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
$gui = $summary | Where-Object { $_.Name -eq 'gui-window' }
$single = $summary | Where-Object { $_.Name -eq 'single-instance' }
$log = $summary | Where-Object { $_.Name -eq 'startup-log' }

$doctorOk = [bool]$doctor.ok -and ($doctor.diagnostics.checks.Count -ge 6)
Row 'C23' 'doctor self-check' $doctorOk ("checks={0} java={1} webview2={2}" -f $doctor.diagnostics.checks.Count, $doctor.diagnostics.javaCount, $doctor.diagnostics.webview2)

$singleOk = [bool]$single.Pass
$logOk = [bool]$log.Pass
Row 'C24' 'single instance + startup log' ($singleOk -and $logOk) ("{0}; {1}" -f $single.Detail, $log.Detail)

Copy-Item (Join-Path $sc 'summary.json') (Join-Path $acc 'ops-summary.json') -Force -ErrorAction SilentlyContinue
Copy-Item (Join-Path $sc 'doctor.json') (Join-Path $acc 'p40-doctor.json') -Force -ErrorAction SilentlyContinue
$rows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $acc 'summary-ops.json') -Encoding utf8
$pass = @($rows | Where-Object { $_.Pass }).Count
Write-Host ("OPS {0}/{1} passed" -f $pass, $rows.Count)
