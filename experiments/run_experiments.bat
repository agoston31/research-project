@echo off
setlocal enabledelayedexpansion

set OUTDIR=results\articulation-extra

if not exist %OUTDIR% mkdir %OUTDIR%

for /R instances %%f in (*.dzn) do (
    set "folder=%%~dpf"
    for %%a in ("%%~dpf.") do set "parent=%%~nxa"

    echo Running %%f

    minizinc --solver "pumpkin-circuit" ^
        -v ^
        --time-limit 900000 ^
        --statistics ^
        models\circuit_model.mzn ^
        "%%f" ^
        > "%OUTDIR%\!parent!_%%~nf.txt" 2>&1

)