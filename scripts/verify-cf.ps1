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

$keyFile = Join-Path $data 'cf-key.txt'
if (-not (Test-Path $keyFile)) {
    Row 'C29' 'CurseForge detail/files' $false 'missing .data/cf-key.txt'
    Row 'C30' 'CurseForge download sha1' $false 'skipped'
    Row 'C31' 'CurseForge batch files' $false 'skipped'
    Row 'C32' 'CurseForge search' $false 'skipped'
} else {
    $env:CURSEFORGE_API_KEY = ([IO.File]::ReadAllText($keyFile)).Trim()

    # C29 项目详情 + 文件列表
    $p = & $cli --data $data cf-project 238222 2>$null | Out-String
    $f = & $cli --data $data cf-files 238222 --limit 3 2>$null | Out-String
    $ok29 = $false; $d29 = 'parse error'
    try {
        $pj = ($p | ConvertFrom-Json).project
        $fl = ($f | ConvertFrom-Json)
        $ok29 = ($pj.id -eq 238222) -and ($fl.count -ge 1)
        $d29 = "project=$($pj.name) files=$($fl.count) first=$($fl.files[0].fileName)"
    } catch { $d29 = ($p + $f).Trim() }
    Row 'C29' 'CurseForge detail/files' $ok29 $d29

    # C30 单文件下载 + SHA1
    $fileId = 9255357
    $expected = 'cf661b88cbbeb04f3a1194b7c71bffd9add464c4'
    $dest = Join-Path $data 'cf-test\verify-jei.jar'
    New-Item -ItemType Directory -Force -Path (Split-Path $dest) | Out-Null
    Remove-Item $dest -Force -ErrorAction SilentlyContinue
    $dl = & $cli --data $data cf-download 238222 $fileId $dest 2>$null | Out-String
    $ok30 = $false; $d30 = 'download failed'
    try {
        $j = $dl | ConvertFrom-Json
        $ok30 = $j.ok -and ($j.sha1 -eq $expected)
        $d30 = "size=$($j.size) sha1=$($j.sha1)"
        Copy-Item $dl (Join-Path $acc 'cf-download.json') -Force -ErrorAction SilentlyContinue
    } catch { $d30 = $dl.Trim() }
    Row 'C30' 'CurseForge download sha1' $ok30 $d30

    # C31 批量文件（整合包 manifest 用）
    $b = & $cli --data $data cf-batch 9255357,9255810 2>$null | Out-String
    $ok31 = $false; $d31 = 'batch failed'
    try {
        $bj = $b | ConvertFrom-Json
        $ok31 = ($bj.count -eq 2)
        $d31 = "count=$($bj.count)"
    } catch { $d31 = $b.Trim() }
    Row 'C31' 'CurseForge batch files' $ok31 $d31

    # C32 搜索（有权限则校验结果；无权限应返回可读错误而不是空结果）
    $searchOut = Join-Path $env:TEMP "mica-cf-search-verify.json"
    & $cli --data $data cf-search modpack 'project infinity' --limit 3 2>$null | Set-Content -Path $searchOut -Encoding UTF8
    $s = if (Test-Path $searchOut) { Get-Content $searchOut -Raw -Encoding UTF8 } else { '' }
    if (-not $s) { $s = (& $cli --data $data cf-search modpack 'project infinity' --limit 3 2>$null | Out-String) }
    $ok32 = $false; $d32 = 'search failed'
    try {
        $sj = $s | ConvertFrom-Json
        if ($sj.ok) {
            $ok32 = ($sj.total -gt 0)
            $d32 = "total=$($sj.total) first=$($sj.projects[0].name)"
        } elseif ($sj.error -match '搜索权限') {
            $ok32 = $true
            $d32 = 'no-search-permission (readable error)'
        } else { $d32 = $sj.error }
    } catch { $d32 = $s.Trim() }
    Row 'C32' 'CurseForge search' $ok32 $d32
}

$rows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $acc 'summary-cf.json') -Encoding utf8
Write-Host ("CF {0}/{1} passed" -f @($rows | Where-Object { $_.Pass }).Count, $rows.Count)
