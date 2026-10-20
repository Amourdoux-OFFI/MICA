$ErrorActionPreference = 'Continue'
$root = 'D:\Desktop\DSH\MICA'
$data = Join-Path $root '.data'
$acc  = Join-Path $data 'acceptance'
$cli  = Join-Path $root 'src-tauri\target\debug\mica-cli.exe'
$rows = @()
function Row([string]$id, [string]$name, [bool]$ok, [string]$detail) {
    $script:rows += [pscustomobject]@{ Id = $id; Name = $name; Pass = $ok; Evidence = $detail }
    $flag = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("[{0}] {1} {2} :: {3}" -f $flag, $id, $name, $detail)
}

# 前端真实调用 modrinth_search 时发送的 payload（camelCase，来自 api.ts）
$payload = '{"query":"sodium","projectType":"mod","categories":["fabric"],"loaders":[],"gameVersions":["1.20.1"],"sort":"relevance","offset":0,"limit":1}'
$file = Join-Path $data 'search-payload.json'
Set-Content -Path $file -Value $payload -NoNewline -Encoding ascii

$out = & $cli --data $data search-json $file 2>$null | Out-String
$ok = $false
$detail = 'no output'
try {
    $j = $out | ConvertFrom-Json
    $ok = [bool]$j.ok -and ($j.total -gt 0) -and ($j.first -eq 'sodium')
    $detail = "total={0} first={1}" -f $j.total, $j.first
} catch {
    $detail = $out.Trim()
}
Row 'C28' 'frontend payload accepted by command boundary' $ok $detail

$rows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $acc 'summary-ipc.json') -Encoding utf8
Write-Host ("IPC {0}/{1} passed" -f @($rows | Where-Object { $_.Pass }).Count, $rows.Count)
