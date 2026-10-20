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

$sizeMb = 0
if (Test-Path $exe) { $sizeMb = [math]::Round((Get-Item $exe).Length / 1MB, 1) }
$dlls = @(Get-ChildItem $rel -Filter *.dll -ErrorAction SilentlyContinue).Count

# 无窗口自检（证明 release 二进制可运行，并自证内置 WebView2 引导器）
$selftest = Join-Path $rel 'MICA-Data\logs\selftest.txt'
if (Test-Path $selftest) { Remove-Item $selftest -Force }
cmd /c "start /wait `"`" `"$exe`" --self-test" | Out-Null

$selfOk = $false
$bootBytes = 0
$bootHash = ''
if (Test-Path $selftest) {
    $text = [IO.File]::ReadAllText($selftest, [Text.Encoding]::UTF8)
    $m1 = [regex]::Match($text, 'bootstrapper_bytes=(\d+)')
    $m2 = [regex]::Match($text, 'bootstrapper_sha256=([0-9a-f]{64})')
    if ($m1.Success) { $bootBytes = [int64]$m1.Groups[1].Value }
    if ($m2.Success) { $bootHash = $m2.Groups[1].Value }
    $selfOk = ($text -match 'webview2=')
}
$expected = 'f414de01c4680f0bb0694d1ea7803c6968f3a81bf6bad01b89bfc65dce14c2a2'
$hashOk = ($bootHash -eq $expected)

# 不依赖 VC++ 运行库：解析 PE 导入表（link /dump），避免被内嵌的 WebView2 安装器误判
$link = 'C:\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe'
$needsVcruntime = $false
if (Test-Path $link) {
    $dump = (& $link /dump /dependents $exe 2>&1 | Out-String)
    $needsVcruntime = ($dump -match 'VCRUNTIME|MSVCP140')
}

$ok = (Test-Path $exe) -and ($dlls -eq 0) -and ($bootBytes -gt 1000000) -and $hashOk -and (-not $needsVcruntime) -and ($sizeMb -lt 30) -and $selfOk
$detail = "size={0}MB dlls={1} bootstrapper={2}B hashOk={3} vcruntime={4} selftest={5}" -f $sizeMb, $dlls, $bootBytes, $hashOk, $needsVcruntime, $selfOk
Row 'C26' 'single exe / zero dependency' $ok $detail

$rows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $acc 'summary-single.json') -Encoding utf8
Write-Host ("SINGLE {0}/{1} passed" -f @($rows | Where-Object { $_.Pass }).Count, $rows.Count)
