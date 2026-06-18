import os
import re
from collections import defaultdict
from statistics import mean

BASE_RESULTS_DIR = "results"
SOLVER_DIRS = {
    "basic": os.path.join(BASE_RESULTS_DIR, "basic-extra"),
    "articulation": os.path.join(BASE_RESULTS_DIR, "strong-articulation-extra"),
}

STAT_PATTERNS = {
    "solveTime": r"solveTime=([0-9.]+)",
    "nodes": r"nodes=([0-9]+)",
    "failures": r"failures=([0-9]+)",
    "propagations": r"propagations=([0-9]+)",
    "average_learned_nogood_length": r"AverageLearnedNogoodLength=([0-9.]+)",
    "num_strong_articulation_points": r"NumStrongArticulationPoints=([0-9]+)",
    "num_strong_articulation_prunings": r"NumArticulationPrunings=([0-9]+)",
    "num_strong_articulation_conflicts": r"NumStrongArticulationConflicts=([0-9]+)",
}

NAME_PATTERN = r"n(\d+)_k(\d+)_.*?seed(\d+)"


def is_timeout(content: str) -> bool:
    return "=====UNKNOWN=====" in content


def extract_stat(content: str, stat_name: str):
    matches = re.findall(STAT_PATTERNS[stat_name], content)

    if not matches:
        return None

    value = matches[-1]

    if stat_name in {"solveTime", "average_learned_nogood_length"}:
        return float(value)

    return int(value)


def read_solver_results(results_dir: str):
    results = {}

    for filename in os.listdir(results_dir):
        if not filename.endswith(".txt"):
            continue

        match = re.search(NAME_PATTERN, filename)
        if not match:
            print(f"Skipping malformed filename: {filename}")
            continue

        n = int(match.group(1))
        k = int(match.group(2))
        seed = int(match.group(3))
        instance_key = (n, k, seed)

        filepath = os.path.join(results_dir, filename)

        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()

        result = {
            "timeout": is_timeout(content),
            "solveTime": extract_stat(content, "solveTime"),
            "nodes": extract_stat(content, "nodes"),
            "failures": extract_stat(content, "failures"),
            "propagations": extract_stat(content, "propagations"),
            "average_learned_nogood_length": extract_stat(content, "average_learned_nogood_length"),
            "num_strong_articulation_points": extract_stat(content, "num_strong_articulation_points"),
            "num_strong_articulation_prunings": extract_stat(content, "num_strong_articulation_prunings"),
            "num_strong_articulation_conflicts": extract_stat(content, "num_strong_articulation_conflicts"),
        }

        results[instance_key] = result

    return results


def empty_group():
    return {
        "solveTime": [],
        "nodes": [],
        "failures": [],
        "propagations": [],
        "average_learned_nogood_length": [],
        "num_strong_articulation_points": [],
        "num_strong_articulation_prunings": [],
        "num_strong_articulation_conflicts": [],
        "timeouts": 0,
        "total": 0,
        "included": 0,
    }


all_results = {
    solver_name: read_solver_results(results_dir)
    for solver_name, results_dir in SOLVER_DIRS.items()
}

common_instances = set.intersection(
    *(set(results.keys()) for results in all_results.values())
)

excluded_instances = set()

for instance_key in common_instances:
    if any(all_results[solver][instance_key]["timeout"] for solver in SOLVER_DIRS):
        excluded_instances.add(instance_key)

included_instances = common_instances - excluded_instances

grouped = {
    solver_name: defaultdict(empty_group)
    for solver_name in SOLVER_DIRS
}

STAT_NAMES = [
    "solveTime",
    "nodes",
    "failures",
    "propagations",
    "average_learned_nogood_length",
    "num_strong_articulation_points",
    "num_strong_articulation_prunings",
    "num_strong_articulation_conflicts",
]

for solver_name, solver_results in all_results.items():
    for instance_key in common_instances:
        n, k, seed = instance_key
        group_key = (n, k)
        result = solver_results[instance_key]

        grouped[solver_name][group_key]["total"] += 1

        if result["timeout"]:
            grouped[solver_name][group_key]["timeouts"] += 1

        if instance_key in excluded_instances:
            continue

        grouped[solver_name][group_key]["included"] += 1

        for stat_name in STAT_NAMES:
            value = result[stat_name]
            if value is not None:
                grouped[solver_name][group_key][stat_name].append(value)


def avg(values):
    return mean(values) if values else 0


def print_summary(solver_name: str, groups):
    print(f"\n===== {solver_name.upper()} SUMMARY =====\n")

    header = (
        f"{'n':>5} {'k':>5} "
        f"{'runs':>6} "
        f"{'timeouts':>9} "
        f"{'included':>9} "
        f"{'time_avg':>12} "
        f"{'nodes_avg':>12} "
        f"{'fail_avg':>12} "
        f"{'prop_avg':>12} "
        f"{'nogood_len_avg':>16} "
        f"{'sap_avg':>12} "
        f"{'sap_prune_avg':>15}"
        f"{'sap_conf_avg':>15}"
    )

    print(header)
    print("-" * len(header))

    for (n, k) in sorted(groups.keys()):
        stats = groups[(n, k)]

        print(
            f"{n:5d} {k:5d} "
            f"{stats['total']:6d} "
            f"{stats['timeouts']:9d} "
            f"{stats['included']:9d} "
            f"{avg(stats['solveTime']):12.4f} "
            f"{avg(stats['nodes']):12.2f} "
            f"{avg(stats['failures']):12.2f} "
            f"{avg(stats['propagations']):12.2f} "
            f"{avg(stats['average_learned_nogood_length']):16.2f} "
            f"{avg(stats['num_strong_articulation_points']):12.2f} "
            f"{avg(stats['num_strong_articulation_prunings']):15.2f}"
            f"{avg(stats['num_strong_articulation_conflicts']):15.2f}"
        )


print(f"\nCommon instances: {len(common_instances)}")
print(f"Excluded because at least one solver timed out: {len(excluded_instances)}")
print(f"Included in averages: {len(included_instances)}")

print_summary("basic", grouped["basic"])
print_summary("articulation", grouped["articulation"])