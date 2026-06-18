@echo off

REM Graph sizes
set SIZES=20 30 50 100

REM Average outgoing degree
set DEGREES=3 4 5 7 10

REM Number of random seeds per configuration
set NUM_SEEDS=20

for %%n in (%SIZES%) do (
    for %%k in (%DEGREES%) do (
        for /L %%s in (1,1,%NUM_SEEDS%) do (

            echo Generating n=%%n k=%%k seed=%%s

            python generate.py %%n %%k %%s
        )
    )
)

echo Done.
pause