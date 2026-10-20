# 载入 MSVC 开发者环境 + Rust 工具链路径（仅用于本机构建）
$envFile = 'D:\tools\vcvars64.env'
if (Test-Path $envFile) {
    foreach ($line in Get-Content $envFile) {
        if ($line -match '^([^=]+)=(.*)$') {
            Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
        }
    }
}
$env:RUSTUP_HOME = 'D:\tools\rustup'
$env:CARGO_HOME = 'D:\tools\cargo'
$env:Path = "D:\tools\cargo\bin;$env:Path"
