import os
import re
from collections import defaultdict
from statistics import mean

RESULTS_DIR = "results/articulation"

# Regex patterns for MiniZinc stats
STAT_PATTERNS = {
    "solveTime": r"solveTime=([0-9.]+)",
    "nodes": r"nodes=([0-9]+)",
    "failures": r"failures=([0-9]+)",
    "propagations": r"propagations=([0-9]+)",
}

# Extract n and k from filenames like:
# n50_k5_seed3_instance.txt
NAME_PATTERN = r"n(\d+)_k(\d+)_"

# Store grouped statistics
grouped = defaultdict(lambda: defaultdict(list))

for filename in os.listdir(RESULTS_DIR):

    if not filename.endswith(".txt"):
        continue

    filepath = os.path.join(RESULTS_DIR, filename)

    match = re.search(NAME_PATTERN, filename)

    if not match:
        print(f"Skipping malformed filename: {filename}")
        continue

    n = int(match.group(1))
    k = int(match.group(2))

    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    for stat_name, pattern in STAT_PATTERNS.items():
        stat_match = re.search(pattern, content)

        if stat_match:
            value = float(stat_match.group(1))
            grouped[(n, k)][stat_name].append(value)

# Print summary table
print("\n===== SUMMARY =====\n")

header = (
    f"{'n':>5} {'k':>5} "
    f"{'time_avg':>12} "
    f"{'nodes_avg':>12} "
    f"{'fail_avg':>12} "
    f"{'prop_avg':>12}"
)

print(header)
print("-" * len(header))

for (n, k) in sorted(grouped.keys()):

    stats = grouped[(n, k)]

    time_avg = mean(stats["solveTime"]) if stats["solveTime"] else 0
    nodes_avg = mean(stats["nodes"]) if stats["nodes"] else 0
    fail_avg = mean(stats["failures"]) if stats["failures"] else 0
    prop_avg = mean(stats["propagations"]) if stats["propagations"] else 0

    print(
        f"{n:5d} {k:5d} "
        f"{time_avg:12.4f} "
        f"{nodes_avg:12.2f} "
        f"{fail_avg:12.2f} "
        f"{prop_avg:12.2f}"
    )