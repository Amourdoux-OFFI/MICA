@echo off
rem Inject MSVC + Rust toolchain environment, then run the given command.
call "C:\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set "RUSTUP_HOME=D:\tools\rustup"
set "CARGO_HOME=D:\tools\cargo"
set "PATH=D:\tools\cargo\bin;%PATH%"
%*
