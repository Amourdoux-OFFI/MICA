$ErrorActionPreference = 'Continue'
$Data = 'D:\Desktop\DSH\MICA\.data'
$Acc  = Join-Path $Data 'acceptance'
$exe  = 'D:\Desktop\DSH\MICA\src-tauri\target\debug\mica-cli.exe'
$script:results = @()

function Add([string]$id, [string]$name, [bool]$ok, [string]$evidence) {
    $script:results += [pscustomobject]@{ Id = $id; Name = $name; Pass = $ok; Evidence = $evidence }
    $flag = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("{0} {1} {2} :: {3}" -f $flag, $id, $name, $evidence)
}
function Json([string]$f) { $p = Join-Path $Acc $f; if (Test-Path $p) { [IO.File]::ReadAllText($p, [Text.Encoding]::UTF8) | ConvertFrom-Json } }
function Run([string]$name, [string[]]$cliArgs) {
    & $exe --data $Data @cliArgs 1> (Join-Path $Acc "$name.json") 2> (Join-Path $Acc "$name.err.log")
}

# C21 server
$det = Json 'p30-server-detect.json'
$ins = Json 'p31-server-install.json'
$run = Json 'p32-server-start.json'
$srvId = $ins.report.server.id
$srvDir = Join-Path $Data "servers\$srvId"
$hasJar = (Test-Path (Join-Path $srvDir 'server.jar')) -or (Test-Path (Join-Path $srvDir 'fabric-server-launch.jar'))
$hasProps = Test-Path (Join-Path $srvDir 'server.properties')
$hasMeta = Test-Path (Join-Path $srvDir 'server.json')
$ok21 = [bool]$det.ok -and [bool]$det.server.supported -and [bool]$ins.ok -and $hasJar -and $hasProps -and $hasMeta -and [bool]$run.ready
Add 'C21' 'server pack install + start' $ok21 ("detect={0} install={1} id={2} jar={3} props={4} ready={5}" -f $det.server.supported,$ins.ok,$srvId,$hasJar,$hasProps,$run.ready)

# C22 localization (deterministic: reset known original first)
$game = Join-Path $Data 'instances\1.20.1-fabric\game'
Set-Content (Join-Path $game 'config\demo.txt') 'ORIGINAL' -NoNewline
$rpDir = Join-Path $game 'resourcepacks'
Get-ChildItem $rpDir -File -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
Get-ChildItem (Join-Path $Data 'instances\1.20.1-fabric\backups') -Directory -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Set-Content (Join-Path $Data 'instances\1.20.1-fabric\localizations.json') '[]'

Run 'p37-l10n-apply' @('localize', '1.20.1-fabric', (Join-Path $Data 'testpacks\l10n.zip'))
$apply = Json 'p37-l10n-apply.json'
$rec = $apply.report.record
$backupFile = Join-Path (Join-Path $Data 'instances\1.20.1-fabric') (Join-Path $rec.backup 'config\demo.txt')
$backupOriginal = (Get-Content $backupFile -Raw -ErrorAction SilentlyContinue).Trim()
$afterApply = (Get-Content (Join-Path $game 'config\demo.txt') -Raw).Trim()
$addedAfter = @(Get-ChildItem $rpDir -File -ErrorAction SilentlyContinue).Count

Run 'p38-l10n-rollback' @('localize-rollback', '1.20.1-fabric', $rec.id)
$roll = Json 'p38-l10n-rollback.json'
$afterRollback = (Get-Content (Join-Path $game 'config\demo.txt') -Raw).Trim()
$addedAfterRollback = @(Get-ChildItem $rpDir -File -ErrorAction SilentlyContinue).Count
$remainingRaw = (Get-Content (Join-Path $Data 'instances\1.20.1-fabric\localizations.json') -Raw -ErrorAction SilentlyContinue).Trim()
$remaining = if ($remainingRaw -eq '[]' -or $remainingRaw -eq '') { 0 } else { 1 }

Run 'p39-l10n-reapply' @('localize', '1.20.1-fabric', (Join-Path $Data 'testpacks\l10n.zip'))
$re = Json 'p39-l10n-reapply.json'

$ok22 = [bool]$apply.ok -and ($rec.replaced -ge 1) -and ($rec.added -ge 1) -and ($backupOriginal -eq 'ORIGINAL') -and ($afterApply -ne 'ORIGINAL') -and ($addedAfter -ge 1) -and [bool]$roll.ok -and ($afterRollback -eq 'ORIGINAL') -and ($addedAfterRollback -eq 0) -and ($remaining -eq 0) -and [bool]$re.ok
Add 'C22' 'localization apply/backup/rollback' $ok22 ("replaced={0} added={1} backup={2} applied={3} rollback={4} restored={5} addedRemoved={6} reapply={7}" -f $rec.replaced,$rec.added,$backupOriginal,$afterApply,$roll.ok,$afterRollback,(-not $addedAfterRollback),$re.ok)

$script:results | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Acc 'summary-extra.json') -Encoding utf8
$pass = @($script:results | Where-Object { $_.Pass }).Count
Write-Host ("EXTRA SUMMARY {0}/{1} passed" -f $pass, $script:results.Count)
