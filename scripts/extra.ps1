$ErrorActionPreference = 'Continue'
$Data = 'D:\Desktop\DSH\MICA\.data'
$Acc  = Join-Path $Data 'acceptance'
$exe  = 'D:\Desktop\DSH\MICA\src-tauri\target\debug\mica-cli.exe'
New-Item -ItemType Directory -Force -Path $Acc | Out-Null

function Run([string]$name, [string[]]$cliArgs) {
    Write-Host "==> $name"
    & $exe --data $Data @cliArgs 1> (Join-Path $Acc "$name.json") 2> (Join-Path $Acc "$name.err.log")
    Write-Host "    exit=$LASTEXITCODE"
}

Run 'p30-server-detect' @('server-detect', (Join-Path $Data 'testpacks\server.zip'))
Run 'p31-server-install' @('server-install', (Join-Path $Data 'testpacks\server.zip'), '--mc', '1.20.1', '--loader', 'vanilla', '--eula', '--name', 'MICA Verify Server')
Run 'p32-server-start' @('server-start', 'MICA-Verify-Server', '--seconds', '75')

Run 'p33-localize' @('localize', '1.20.1-fabric', (Join-Path $Data 'testpacks\l10n.zip'))
Run 'p34-localize-list' @('localize-list', '1.20.1-fabric')
$rec = (Get-Content (Join-Path $Acc 'p33-localize.json') -Raw | ConvertFrom-Json).report.record
Run 'p35-localize-rollback' @('localize-rollback', '1.20.1-fabric', $rec.id)
Run 'p36-localize-reapply' @('localize', '1.20.1-fabric', (Join-Path $Data 'testpacks\l10n.zip'))
Write-Host 'extra done'
