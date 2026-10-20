$ErrorActionPreference = 'Continue'
$root = 'D:\Desktop\DSH\MICA'
$rel  = Join-Path $root 'release'
$exe  = Join-Path $rel 'MICA.exe'
$data = Join-Path $root '.data'
$acc  = Join-Path $data 'acceptance'
$rows = @()
function Row([string]$id, [string]$name, [bool]$ok, [string]$detail) {
    $script:rows += [pscustomobject]@{ Id = $id; Name = $name; Pass = $ok; Evidence = $detail }
    $flag = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("[{0}] {1} {2} :: {3}" -f $flag, $id, $name, $detail)
}

# Hidden-window check: packaged build must load embedded assets, not the dev server.
$logDir = Join-Path $rel 'MICA-Data\logs'
$selftest = Join-Path $logDir 'selftest-web.txt'
if (Test-Path $selftest) { Remove-Item $selftest -Force }

& $exe --self-test-web | Out-Null
$deadline = (Get-Date).AddSeconds(15)
while (-not (Test-Path $selftest) -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 300
}
# wait for the app to finish writing/exiting
Start-Sleep -Milliseconds 800

$url = ''
$title = ''
$embedded = $false
$okFlag = $false
if (Test-Path $selftest) {
    $text = [IO.File]::ReadAllText($selftest, [Text.Encoding]::UTF8)
    if ($text -match 'web_url=(\S+)') { $url = $Matches[1] }
    if ($text -match 'web_title=(.+)') { $title = $Matches[1].Trim() }
    $embedded = ($text -match 'embedded=true')
    $okFlag = ($text -match 'ok=true')
    Copy-Item $selftest (Join-Path $acc 'p41-selftest-web.txt') -Force -ErrorAction SilentlyContinue
}

$ok = $embedded -and $okFlag -and ($title -eq 'MICA') -and ($url -notmatch 'localhost:5173')
Row 'C27' 'packaged webview loads embedded assets' $ok ("url={0} title={1} embedded={2}" -f $url, $title, $embedded)

$rows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $acc 'summary-web.json') -Encoding utf8
Write-Host ("WEB {0}/{1} passed" -f @($rows | Where-Object { $_.Pass }).Count, $rows.Count)
