$ErrorActionPreference = 'Continue'
$Data = 'D:\Desktop\DSH\MICA\.data'
$Acc  = Join-Path $Data 'acceptance'
$exe  = 'D:\Desktop\DSH\MICA\src-tauri\target\debug\mica-cli.exe'

function Run([string]$name, [string[]]$cliArgs) {
    Write-Host "==> $name"
    & $exe --data $Data @cliArgs 1> (Join-Path $Acc "$name.json") 2> (Join-Path $Acc "$name.err.log")
    Write-Host "    exit=$LASTEXITCODE"
}

Run 'p06b-install-fabric' @('install-fabric', '1.20.1', '--loader', '0.16.10', '--instance', '1.20.1-fabric')
Run 'p13-install-mod' @('install-mod', '1.20.1-fabric', 'sodium', '--kind', 'mod')
Run 'p14-install-rp' @('install-mod', '1.20.1-fabric', 'faithful-32x', '--kind', 'resourcepack')
Run 'p15-install-shader' @('install-mod', '1.20.1-fabric', 'complementary-reimagined', '--kind', 'shader')
Run 'p16-install-datapack' @('install-mod', '1.20.1-fabric', 'terralith', '--kind', 'datapack')
Run 'p25-install-generic' @('install-pack', (Join-Path $Data 'testpacks\generic.zip'), '--instance', '1.20.1-fabric')
Run 'p20-launch-fabric' @('launch', '1.20.1-fabric', '--seconds', '70')
Write-Host 'finish done'
