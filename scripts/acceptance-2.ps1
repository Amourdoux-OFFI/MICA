#Requires -Version 5.1
param(
    [string]$Data = 'D:\Desktop\DSH\MICA\.data',
    [string]$Out  = 'D:\Desktop\DSH\MICA\.data\acceptance'
)
$ErrorActionPreference = 'Continue'
$exe = 'D:\Desktop\DSH\MICA\src-tauri\target\debug\mica-cli.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null

function Run([string]$name, [string[]]$cliArgs) {
    Write-Host "==> $name"
    $log = Join-Path $Out "$name.json"
    $err = Join-Path $Out "$name.err.log"
    & $exe --data $Data @cliArgs 1> $log 2> $err
    Write-Host "    exit=$LASTEXITCODE -> $log"
}

if (Test-Path (Join-Path $Data 'versions\1.20.1-forge-47.2.0')) {
    Remove-Item -Recurse -Force (Join-Path $Data 'versions\1.20.1-forge-47.2.0') -ErrorAction SilentlyContinue
}

Run 'p07-install-forge' @('install-forge', '1.20.1', '--loader', '47.2.0', '--instance', '1.20.1-forge')
Run 'p19-launch-vanilla' @('launch', '1.20.1', '--seconds', '70')
Run 'p13-install-mod' @('install-mod', '1.20.1-fabric', 'sodium', '--kind', 'mod')
Run 'p14-install-rp' @('install-mod', '1.20.1-fabric', 'faithful-32x', '--kind', 'resourcepack')
Run 'p15-install-shader' @('install-mod', '1.20.1-fabric', 'complementary-reimagined', '--kind', 'shader')
Run 'p16-install-datapack' @('install-mod', '1.20.1-fabric', 'terralith', '--kind', 'datapack')
Run 'p20-launch-fabric' @('launch', '1.20.1-fabric', '--seconds', '70')
Run 'p21-modpack-remote' @('install-modpack-remote', 'fabulously-optimized')

$packs = Join-Path $Data 'testpacks'
New-Item -ItemType Directory -Force -Path $packs | Out-Null
$g = Join-Path $packs 'generic'
New-Item -ItemType Directory -Force -Path (Join-Path $g 'mods') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $g 'config') | Out-Null
Set-Content (Join-Path $g 'mods\demo.jar') 'x' -NoNewline
Set-Content (Join-Path $g 'config\demo.txt') 'hello'
Compress-Archive -Path (Join-Path $g '*') -DestinationPath (Join-Path $packs 'generic.zip') -Force

$m = Join-Path $packs 'mmc'
New-Item -ItemType Directory -Force -Path (Join-Path $m '.minecraft\mods') | Out-Null
Set-Content (Join-Path $m 'mmc-pack.json') '{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.10"}]}'
Set-Content (Join-Path $m 'instance.cfg') 'name=MMC Test'
Set-Content (Join-Path $m '.minecraft\mods\demo.jar') 'x' -NoNewline
Compress-Archive -Path (Join-Path $m '*') -DestinationPath (Join-Path $packs 'multimc.zip') -Force

$c = Join-Path $packs 'cf'
New-Item -ItemType Directory -Force -Path (Join-Path $c 'overrides') | Out-Null
Set-Content (Join-Path $c 'manifest.json') '{"manifestType":"minecraftModpack","manifestVersion":1,"name":"CF Test","version":"1.0.0","author":"test","minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.2.0","primary":true}]},"files":[]}'
Compress-Archive -Path (Join-Path $c '*') -DestinationPath (Join-Path $packs 'curseforge.zip') -Force

Run 'p22-detect-generic' @('detect-pack', (Join-Path $packs 'generic.zip'))
Run 'p23-detect-multimc' @('detect-pack', (Join-Path $packs 'multimc.zip'))
Run 'p24-detect-cf' @('detect-pack', (Join-Path $packs 'curseforge.zip'))
Run 'p25-install-generic' @('install-pack', (Join-Path $packs 'generic.zip'), '--instance', '1.20.1-fabric')
Run 'p26-install-cf' @('install-pack', (Join-Path $packs 'curseforge.zip'))

Write-Host 'done'
