#Requires -Version 5.1
param(
    [string]$Data = 'D:\Desktop\DSH\MICA\.data',
    [string]$Out  = 'D:\Desktop\DSH\MICA\.data\acceptance'
)
$ErrorActionPreference = 'Continue'
$exe = 'D:\Desktop\DSH\MICA\src-tauri\target\debug\mica-cli.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null

function Run([string]$name, [string[]]$cliArgs) {
    Write-Host "==> $name" -ForegroundColor Cyan
    $log = Join-Path $Out "$name.json"
    $err = Join-Path $Out "$name.err.log"
    & $exe --data $Data @cliArgs 1> $log 2> $err
    $code = $LASTEXITCODE
    Write-Host "    exit=$code  -> $log"
    return $code
}

# 1. 环境与清单
Run 'p01-info' @('info') | Out-Null
Run 'p02-manifest' @('manifest') | Out-Null

# 2. 加载器版本列表
Run 'p03-fabric-versions' @('fabric-versions', '1.20.1') | Out-Null
Run 'p04-forge-versions' @('forge-versions', '1.20.1') | Out-Null
Run 'p05-neoforge-versions' @('neoforge-versions') | Out-Null

# 3. 安装 Fabric / Forge
Run 'p06-install-fabric' @('install-fabric', '1.20.1', '--loader', '0.16.10', '--instance', '1.20.1-fabric') | Out-Null
Run 'p07-install-forge' @('install-forge', '1.20.1', '--loader', '47.2.0', '--instance', '1.20.1-forge') | Out-Null

# 4. 内容搜索
Run 'p08-search-mod' @('search', 'mod', 'sodium', '--game-version', '1.20.1', '--loader', 'fabric', '--limit', '3') | Out-Null
Run 'p09-search-modpack' @('search', 'modpack', '', '--limit', '2') | Out-Null
Run 'p10-search-rp' @('search', 'resourcepack', '', '--limit', '2') | Out-Null
Run 'p11-search-shader' @('search', 'shader', '', '--limit', '2') | Out-Null
Run 'p12-search-datapack' @('search', 'datapack', '', '--limit', '2') | Out-Null

# 5. 安装内容
Run 'p13-install-mod' @('install-mod', '1.20.1-fabric', 'sodium', '--kind', 'mod') | Out-Null
Run 'p14-install-rp' @('install-mod', '1.20.1-fabric', 'faithful-32x', '--kind', 'resourcepack') | Out-Null
Run 'p15-install-shader' @('install-mod', '1.20.1-fabric', 'complementary-reimagined', '--kind', 'shader') | Out-Null
Run 'p16-install-datapack' @('install-mod', '1.20.1-fabric', 'terralith', '--kind', 'datapack') | Out-Null

# 6. Java
Run 'p17-java-list' @('java-list') | Out-Null
Run 'p18-java-install' @('java-install', '21') | Out-Null

# 7. 启动（真实拉起 Minecraft，观察 marker）
Run 'p19-launch-vanilla' @('launch', '1.20.1', '--seconds', '70') | Out-Null
Run 'p20-launch-fabric' @('launch', '1.20.1-fabric', '--seconds', '70') | Out-Null

# 8. 整合包（Modrinth 远程）
Run 'p21-modpack-remote' @('install-modpack-remote', 'fabulously-optimized') | Out-Null

# 9. 拖入格式识别 + 通用包覆盖
$packs = 'D:\Desktop\DSH\MICA\.data\testpacks'
New-Item -ItemType Directory -Force -Path $packs | Out-Null
$g = Join-Path $packs 'generic'; New-Item -ItemType Directory -Force -Path "$g\mods","$g\config" | Out-Null
Set-Content "$g\mods\demo.jar" 'x' -NoNewline; Set-Content "$g\config\demo.txt" 'hello'
Compress-Archive -Path "$g\*" -DestinationPath "$packs\generic.zip" -Force
$m = Join-Path $packs 'mmc'; New-Item -ItemType Directory -Force -Path "$m\.minecraft\mods" | Out-Null
Set-Content "$m\mmc-pack.json" '{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.10"}]}'
Set-Content "$m\instance.cfg" 'name=MMC Test'
Set-Content "$m\.minecraft\mods\demo.jar" 'x' -NoNewline
Compress-Archive -Path "$m\*" -DestinationPath "$packs\multimc.zip" -Force
$c = Join-Path $packs 'cf'; New-Item -ItemType Directory -Force -Path "$c\overrides" | Out-Null
Set-Content "$c\manifest.json" '{"manifestType":"minecraftModpack","manifestVersion":1,"name":"CF Test","version":"1.0.0","author":"test","minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.2.0","primary":true}]},"files":[]}'
Compress-Archive -Path "$c\*" -DestinationPath "$packs\curseforge.zip" -Force

Run 'p22-detect-generic' @('detect-pack', "$packs\generic.zip") | Out-Null
Run 'p23-detect-multimc' @('detect-pack', "$packs\multimc.zip") | Out-Null
Run 'p24-detect-cf' @('detect-pack', "$packs\curseforge.zip") | Out-Null
Run 'p25-install-generic' @('install-pack', "$packs\generic.zip", '--instance', '1.20.1-fabric') | Out-Null
Run 'p26-install-cf' @('install-pack', "$packs\curseforge.zip") | Out-Null

Write-Host 'done' -ForegroundColor Green
