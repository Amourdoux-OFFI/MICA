@echo off
set RUST_BACKTRACE=1
set WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--no-sandbox --disable-gpu-sandbox --disable-features=RendererCodeIntegrity
"D:\Desktop\DSH\MICA\src-tauri\target\debug\mica.exe" > "D:\Desktop\DSH\MICA\.data\gui-run3.log" 2>&1
echo exit=%ERRORLEVEL% >> "D:\Desktop\DSH\MICA\.data\gui-run3.log"
