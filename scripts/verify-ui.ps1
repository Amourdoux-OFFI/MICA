$ErrorActionPreference = 'Continue'
$root = 'D:\Desktop\DSH\MICA'
$acc  = Join-Path $root '.data\acceptance'
$rows = @()
function Row([string]$id, [string]$name, [bool]$ok, [string]$detail) {
    $script:rows += [pscustomobject]@{ Id = $id; Name = $name; Pass = $ok; Evidence = $detail }
    $flag = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("[{0}] {1} {2} :: {3}" -f $flag, $id, $name, $detail)
}

$summaryPath = Join-Path $root '.data\ui\summary.json'
$ui = [IO.File]::ReadAllText($summaryPath, [Text.Encoding]::UTF8) | ConvertFrom-Json
$total = @($ui).Count
$pass = @($ui | Where-Object { $_.ok }).Count
$failed = @($ui | Where-Object { -not $_.ok } | ForEach-Object { $_.name }) -join ','
$shots = @(Get-ChildItem (Join-Path $root '.data\ui\*.png') -ErrorAction SilentlyContinue | Where-Object { $_.Name -notlike 'u13b*' }).Count
Row 'C25' 'headless UI checks' ($pass -eq $total -and $total -ge 20) ("{0}/{1} passed, screenshots={2}, failed=[{3}]" -f $pass, $total, $shots, $failed)

$rows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $acc 'summary-ui.json') -Encoding utf8
Write-Host ("UI VERIFY {0}/{1}" -f $pass, $total)
