# Experiments

This directory contains the experimental framework used to generate benchmark instances, run MiniZinc/Pumpkin experiments, summarize solver results, and create evaluation figures.

## Overview

The experimental workflow consists of four steps:

1. Generate benchmark instances.
2. Run the solver on all generated instances.
3. Summarize raw solver outputs into aggregate statistics.
4. Generate tables and figures from the summarized statistics.

The typical workflow is:

```bat
generate_all.bat
run_experiments.bat
python summarize_results_2_solvers.py > stats.txt
python plot_solver_stats.py
```

## Benchmark Instances

Instances are stored in:

```text
instances/
```

Each instance represents a graph used for evaluating the circuit constraint propagators.

The main generation parameters are:

* **n**: number of vertices in the graph
* **k**: target average outgoing degree
* **seed**: random seed used for generation

## Generating Instances

Instances can be generated using:

```bat
generate_all.bat
```

Inside this script, the following parameters can be modified:

```bat
set SIZES=20 30 50 100
set DEGREES=3 4 5 7 10
set NUM_SEEDS=20
```

For each combination of graph size, degree, and seed, the script calls:

```bat
python generate.py <n> <k> <seed>
```

Generated instances are automatically stored in the `instances/` directory.

### Instance Generator

The graph generation logic is implemented in:

```text
generator.py
```

The command-line interface for generating a single instance is:

```text
generate.py
```

Example:

```bat
python generate.py 50 5 12
```

This generates a graph with 50 vertices, target average degree 5, and random seed 12.

## Models

MiniZinc models used in the experiments are stored in:

```text
models/
```

These models define the constraint problem and solver configuration used during evaluation.

## Running Experiments

Experiments can be executed using:

```bat
run_experiments.bat
```

The script traverses the `instances/` directory, runs MiniZinc on every generated instance, and stores the raw solver output in the corresponding results folder.

Current result folders include:

```text
results/articulation/
results/baseline/
results/strong-articulation/
```

Each output file contains the MiniZinc and Pumpkin statistics for a single benchmark instance.

## Cleaning Solver Outputs

The raw MiniZinc output files may contain machine-specific information such as local file paths, solver locations, logging messages, and environment details.

To remove this information while preserving all statistics and solutions, use:

```bat
python clean_outputs.py
```

## Summarizing Results

Aggregate statistics can be computed using:

```bat
python summarize_results_2_solvers.py
```

The script compares two solver configurations and reports aggregate statistics grouped by graph size (`n`) and degree (`k`).

To ensure a fair comparison, any instance for which at least one of the compared solvers timed out is excluded from the reported averages. Timeout counts are still reported separately for each solver.

Reported statistics include:

* Number of runs
* Number of timeouts
* Number of included instances
* Average solving time
* Average search nodes
* Average failures
* Average propagations
* Average number of strong articulation points detected
* Average number of articulation-based prunings
* Average number of articulation-based conflicts

To store the generated summary in a file:

```bat
python summarize_results_2_solvers.py > stats.txt
```

## Plotting Tables and Figures

Tables and figures can be generated using:

```bat
python plot_solver_stats.py
```

The plotting script reads:

```text
stats.txt
```

and generates the corresponding tables and visualizations in:

```text
figures/
```

These plots are intended for inclusion in reports, presentations, and papers.

## Reproducing the Complete Experimental Pipeline

To reproduce all experimental results from scratch:

```bat
generate_all.bat
run_experiments.bat
python clean_outputs.py
python summarize_results_2_solvers.py > stats.txt
python plot_solver_stats.py
```

After completion:

* Generated benchmark instances will be available in `instances/`
* Raw solver outputs will be available in `results/`
* Aggregate statistics will be stored in `stats.txt`
* Tables and figures will be generated in `figures/`
