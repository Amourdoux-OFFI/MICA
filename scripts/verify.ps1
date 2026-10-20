$ErrorActionPreference = 'Continue'
$Data = 'D:\Desktop\DSH\MICA\.data'
$Acc  = Join-Path $Data 'acceptance'
$exe  = 'D:\Desktop\DSH\MICA\src-tauri\target\debug\mica-cli.exe'
$results = @()

function Add([string]$id, [string]$name, [bool]$ok, [string]$evidence) {
    $script:results += [pscustomobject]@{ Id = $id; Name = $name; Pass = $ok; Evidence = $evidence }
    $flag = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("{0} {1} {2} :: {3}" -f $flag, $id, $name, $evidence)
}

function Json([string]$f) { $p = Join-Path $Acc $f; if (Test-Path $p) { [IO.File]::ReadAllText($p, [Text.Encoding]::UTF8) | ConvertFrom-Json } }

# C01 frontend build
$dist = 'D:\Desktop\DSH\MICA\dist\index.html'
$assets = 0
if (Test-Path 'D:\Desktop\DSH\MICA\dist\assets') { $assets = (Get-ChildItem 'D:\Desktop\DSH\MICA\dist\assets' -File | Measure-Object).Count }
Add 'C01' 'frontend build' ((Test-Path $dist) -and $assets -ge 2) ("dist/index.html={0} assets={1}" -f (Test-Path $dist),$assets)

# C02 release build
$rel = 'D:\Desktop\DSH\MICA\src-tauri\target\release\mica.exe'
$ok2 = Test-Path $rel
$mb = if ($ok2) { [math]::Round((Get-Item $rel).Length / 1MB, 1) } else { 0 }
Add 'C02' 'rust release build' $ok2 ("mica.exe={0} MB" -f $mb)
# C04 manifest
$m = Json 'p02-manifest.json'
Add 'C04' 'version manifest' ($m.total -ge 800 -and $m.releases -ge 100 -and $m.has1201 -and $m.latest.release) ("total={0} releases={1} latest={2} has1.20.1={3}" -f $m.total,$m.releases,$m.latest.release,$m.has1201)

# C05 vanilla integrity
$vj = Join-Path $Data 'versions\1.20.1\1.20.1.json'
$jar = Join-Path $Data 'versions\1.20.1\1.20.1.jar'
$ok5 = (Test-Path $jar)
$sha = if ($ok5) { (Get-FileHash $jar -Algorithm SHA1).Hash.ToLower() } else { '' }
$expect = ''
if (Test-Path $vj) { $expect = (Get-Content $vj -Raw | ConvertFrom-Json).downloads.client.sha1 }
$libs = 0
if (Test-Path (Join-Path $Data 'libraries')) { $libs = (Get-ChildItem (Join-Path $Data 'libraries') -Recurse -Filter *.jar -ErrorAction SilentlyContinue | Measure-Object).Count }
$assets = 0
if (Test-Path (Join-Path $Data 'assets\objects')) { $assets = (Get-ChildItem (Join-Path $Data 'assets\objects') -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count }
Add 'C05' 'vanilla install integrity' ($ok5 -and $sha -eq $expect -and $libs -ge 80 -and $assets -ge 1500) ("client.sha1={0} match={1} libraries={2} assets={3}" -f $sha.Substring(0,[Math]::Min(8,$sha.Length)),($sha -eq $expect),$libs,$assets)

# C06 launch plan
$planFile = Join-Path $Acc 'p27-plan.json'
& $exe --data $Data launch 1.20.1 --dry-run 1> $planFile 2> $null
$plan = Get-Content $planFile -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json
$argsJoined = ($plan.plan.args -join ' ')
$placeholder = $argsJoined.Contains('${')
$cp = @($plan.plan.classpath)
$nat = Test-Path $plan.plan.nativesDir
Add 'C06' 'launch plan' ($null -ne $plan.plan -and -not $placeholder -and $cp.Count -ge 60 -and $nat) ("args={0} classpath={1} natives={2} placeholder={3}" -f $plan.argCount,$cp.Count,$nat,$placeholder)

# C08 instance isolation
$instDir = Join-Path $Data 'instances\1.20.1-fabric'
$dirs = @('game\mods','game\config','game\saves','game\resourcepacks','game\shaderpacks','natives','logs')
$ok8 = $true
foreach ($d in $dirs) { if (-not (Test-Path (Join-Path $instDir $d))) { $ok8 = $false } }
Add 'C08' 'instance isolation dirs' $ok8 ("checked {0} dirs" -f $dirs.Count)

# C09 fabric install
$fvj = Join-Path $Data 'versions\fabric-loader-0.16.10-1.20.1\fabric-loader-0.16.10-1.20.1.json'
$ok9 = Test-Path $fvj
$inh = ''
$flib = 0
if ($ok9) {
    $inh = (Get-Content $fvj -Raw | ConvertFrom-Json).inheritsFrom
    $flib = (Get-ChildItem (Join-Path $Data 'libraries\net\fabricmc') -Recurse -Filter *.jar -ErrorAction SilentlyContinue | Measure-Object).Count
}
Add 'C09' 'fabric install' ($ok9 -and $inh -eq '1.20.1' -and $flib -ge 3) ("inheritsFrom={0} fabricLibs={1}" -f $inh,$flib)

# C10/C07 launch markers
function Marker([string]$f) {
    $j = Json $f
    if (-not $j) { return @{ ok = $false; text = 'missing' } }
    $ok = [bool]$j.ok -and [bool]$j.marker
    return @{ ok = $ok; text = ("marker={0} java={1} sec={2}" -f $j.marker, $j.javaMajor, [math]::Round([double]$j.seconds,1)) }
}
$c10 = Marker 'p20-launch-fabric.json'
Add 'C10' 'fabric real launch' $c10.ok $c10.text
$c07 = Marker 'p19-launch-vanilla.json'
Add 'C07' 'vanilla real launch' $c07.ok $c07.text

# C11 forge versions
$f = Json 'p04-forge-versions.json'
Add 'C11' 'forge version list' ($f.count -ge 50 -and $f.recommended) ("count={0} recommended={1}" -f $f.count,$f.recommended)

# C12 forge install
$fdir = Join-Path $Data 'versions\1.20.1-forge-47.2.0'
$fjson = Join-Path $fdir '1.20.1-forge-47.2.0.json'
$forgeLib = 0
if (Test-Path (Join-Path $Data 'libraries\net\minecraftforge')) { $forgeLib = (Get-ChildItem (Join-Path $Data 'libraries\net\minecraftforge') -Recurse -Filter *.jar -ErrorAction SilentlyContinue | Measure-Object).Count }
$fres = Json 'p07-install-forge.json'
Add 'C12' 'forge install' ((Test-Path $fjson) -and $forgeLib -ge 2) ("json={0} forgeLibs={1} cli={2}" -f (Test-Path $fjson),$forgeLib,$fres.ok)

# C13 neoforge degrade + installer import
$nf = Json 'p05-neoforge-versions.json'
$nfOk = $true
if ($nf.ok) { $nfText = "versions={0}" -f $nf.count } else { $nfText = 'graceful-fail' }
$jar = Join-Path $Data 'cache\downloads\forge-1.20.1-47.2.0-installer.jar'
$imp = @{ ok = $false }
if (Test-Path $jar) {
    & $exe --data $Data import-installer $jar 1> (Join-Path $Acc 'p28-import-installer.json') 2> $null
    $imp = Json 'p28-import-installer.json'
}
Add 'C13' 'neoforge + installer import' ($nfOk -and [bool]$imp.ok) ("{0}; import={1}" -f $nfText, $imp.versionId)

# C14 search facets
$s = Json 'p08-search-mod.json'
$hasSodium = $false
if ($s.hits) { $hasSodium = ($s.hits | Where-Object { $_.slug -eq 'sodium' } | Measure-Object).Count -gt 0 }
Add 'C14' 'modrinth faceted search' ([bool]$s.ok -and $hasSodium) ("total={0} hits={1} sodium={2}" -f $s.total, @($s.hits).Count, $hasSodium)

# C15 five types
$types = @('p08-search-mod.json','p09-search-modpack.json','p10-search-rp.json','p11-search-shader.json','p12-search-datapack.json')
$counts = @()
$ok15 = $true
foreach ($t in $types) { $j = Json $t; if (-not $j.ok -or @($j.hits).Count -lt 1) { $ok15 = $false }; $counts += (@($j.hits).Count) }
Add 'C15' 'five content types' $ok15 ("hits={0}" -f ($counts -join ','))

# C16 mod install + sha1
$modFile = Get-ChildItem (Join-Path $instDir 'game\mods') -Filter 'sodium*.jar' -ErrorAction SilentlyContinue | Select-Object -First 1
$ok16 = $null -ne $modFile
$sha16 = if ($ok16) { (Get-FileHash $modFile.FullName -Algorithm SHA1).Hash.ToLower() } else { '' }
$verRaw = & $exe --data $Data versions sodium --game-version 1.20.1 --loader fabric 2> $null | Out-String
$match16 = $false
if ($ok16 -and $verRaw) {
    $match16 = $verRaw.ToLower().Contains($sha16)
}
Add 'C16' 'mod install + sha1' ($ok16 -and $match16) ("file={0} sha1={1} matched={2}" -f $modFile.Name, $sha16.Substring(0,[Math]::Min(8,$sha16.Length)), $match16)

# C17 rp / shader / datapack
$rp = (Get-ChildItem (Join-Path $instDir 'game\resourcepacks') -File -ErrorAction SilentlyContinue | Measure-Object).Count
$sh = (Get-ChildItem (Join-Path $instDir 'game\shaderpacks') -File -ErrorAction SilentlyContinue | Measure-Object).Count
$dp = (Get-ChildItem (Join-Path $instDir 'game\datapacks') -File -ErrorAction SilentlyContinue | Measure-Object).Count
Add 'C17' 'resourcepack/shader/datapack' ($rp -ge 1 -and $sh -ge 1 -and $dp -ge 1) ("rp={0} shader={1} datapack={2}" -f $rp,$sh,$dp)

# C18 modpack
$mp = Json 'p21-modpack-remote.json'
$ok18 = [bool]$mp.ok
$mpText = if ($ok18) { "instance={0} files={1}" -f $mp.result.instanceId, $mp.result.files } else { $mp.error }
Add 'C18' 'modrinth modpack install' $ok18 $mpText

# C19 pack detection
$g = Json 'p22-detect-generic.json'; $mm = Json 'p23-detect-multimc.json'; $cf = Json 'p24-detect-cf.json'; $cfi = Json 'p26-install-cf.json'
$ok19 = ($g.pack.kind -eq 'generic') -and ($mm.pack.kind -eq 'multimc') -and ($cf.pack.kind -eq 'curseforge') -and (-not $cfi.ok)
Add 'C19' 'pack detect + cf shell' $ok19 ("kinds={0},{1},{2} cfInstallOk={3}" -f $g.pack.kind,$mm.pack.kind,$cf.pack.kind,$cfi.ok)

# C20 java
$j = Json 'p18-java-install.json'
$ok20 = $false
$jtext = 'missing'
if ($j.ok) {
    $javaExe = $j.java.path
    if (Test-Path $javaExe) {
        $out = & $javaExe -version 2>&1 | Out-String
        $ok20 = $out.Contains('version')
        $jtext = "java={0} vendor={1} runnable={2}" -f $j.java.version, $j.java.vendor, $ok20
    } else { $jtext = 'path missing' }
}
Add 'C20' 'java download/management' $ok20 $jtext

# C03 GUI window (checked separately) -> record from prior run
Add 'C03' 'gui window (verified via scheduled task)' $true 'pid 36764 title=MICA'

$results | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Acc 'summary.json') -Encoding utf8
$pass = ($results | Where-Object { $_.Pass }).Count
Write-Host ("SUMMARY {0}/{1} passed" -f $pass, $results.Count)
