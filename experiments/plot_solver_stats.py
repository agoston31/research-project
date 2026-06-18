from __future__ import annotations

import argparse
import csv
import math
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import matplotlib.pyplot as plt
from matplotlib.lines import Line2D


@dataclass(frozen=True)
class Row:
    solver: str
    n: int
    k: int
    runs: int
    timeouts: int
    included: int
    time_avg: float
    nodes_avg: float
    fail_avg: float
    prop_avg: float
    nogood_len_avg: float
    sap_avg: float
    sap_prune_avg: float
    sap_conf_avg: float


ROW_RE = re.compile(
    r"^\s*"
    r"(?P<n>\d+)\s+"
    r"(?P<k>\d+)\s+"
    r"(?P<runs>\d+)\s+"
    r"(?P<timeouts>\d+)\s+"
    r"(?P<included>\d+)\s+"
    r"(?P<time_avg>[0-9.]+)\s+"
    r"(?P<nodes_avg>[0-9.]+)\s+"
    r"(?P<fail_avg>[0-9.]+)\s+"
    r"(?P<prop_avg>[0-9.]+)\s+"
    r"(?P<nogood_len_avg>[0-9.]+)\s+"
    r"(?P<sap_avg>[0-9.]+)\s+"
    r"(?P<sap_prune_avg>[0-9.]+)\s+"
    r"(?P<sap_conf_avg>[0-9.]+)\s*$"
)


def parse_stats(path: Path) -> list[Row]:
    rows: list[Row] = []
    current_solver: str | None = None

    for line in path.read_text(encoding="utf-8").splitlines():
        upper = line.upper()

        if "BASIC SUMMARY" in upper:
            current_solver = "CP"
            continue

        if "ARTICULATION SUMMARY" in upper or "SAP SUMMARY" in upper:
            current_solver = "SAP"
            continue

        match = ROW_RE.match(line)
        if not match or current_solver is None:
            continue

        g = match.groupdict()
        rows.append(
            Row(
                solver=current_solver,
                n=int(g["n"]),
                k=int(g["k"]),
                runs=int(g["runs"]),
                timeouts=int(g["timeouts"]),
                included=int(g["included"]),
                time_avg=float(g["time_avg"]),
                nodes_avg=float(g["nodes_avg"]),
                fail_avg=float(g["fail_avg"]),
                prop_avg=float(g["prop_avg"]),
                nogood_len_avg=float(g["nogood_len_avg"]),
                sap_avg=float(g["sap_avg"]),
                sap_prune_avg=float(g["sap_prune_avg"]),
                sap_conf_avg=float(g["sap_conf_avg"]),
            )
        )

    if not rows:
        raise ValueError(f"No summary rows found in {path}")

    solvers = {row.solver for row in rows}
    if "CP" not in solvers or "SAP" not in solvers:
        raise ValueError(
            "Expected both BASIC SUMMARY and ARTICULATION SUMMARY tables in the input."
        )

    return rows


def values_for(rows: list[Row], solver: str, k: int, metric: str) -> tuple[list[int], list[float]]:
    selected = sorted(
        [row for row in rows if row.solver == solver and row.k == k],
        key=lambda row: row.n,
    )
    ns = [row.n for row in selected]
    ys = [getattr(row, metric) for row in selected]
    return ns, ys


def positive_for_log(values: Iterable[float]) -> list[float]:
    # Matplotlib log scale cannot show zero. Use NaN for zero values so they are omitted.
    return [value if value > 0 else math.nan for value in values]


def add_compact_legend(ax, ks: list[int], colors: list[str]) -> None:
    color_handles = [
        Line2D([0], [0], color=colors[i % len(colors)], linewidth=2, label=f"$k={k}$")
        for i, k in enumerate(ks)
    ]

    style_handles = [
        Line2D([0], [0], color="black", linestyle="-", linewidth=2, label="CP"),
        Line2D([0], [0], color="black", linestyle="--", linewidth=2, label="SAP"),
    ]

    ax.legend(
        handles=color_handles + style_handles,
        ncol=4,
        fontsize=8,
        frameon=False,
    )


def plot_metric_by_n(
    rows: list[Row],
    out_dir: Path,
    metric: str,
    ylabel: str,
    filename: str,
    *,
    log_scale: bool = True,
) -> None:
    ks = sorted({row.k for row in rows})
    colors = plt.rcParams["axes.prop_cycle"].by_key()["color"]

    fig, ax = plt.subplots(figsize=(7.2, 4.2))

    for index, k in enumerate(ks):
        color = colors[index % len(colors)]

        cp_ns, cp_values = values_for(rows, "CP", k, metric)
        sap_ns, sap_values = values_for(rows, "SAP", k, metric)

        if log_scale:
            cp_values = positive_for_log(cp_values)
            sap_values = positive_for_log(sap_values)

        ax.plot(
            cp_ns,
            cp_values,
            marker="o",
            linestyle="-",
            linewidth=1.8,
            color=color,
            label="_nolegend_",
        )
        ax.plot(
            sap_ns,
            sap_values,
            marker="o",
            linestyle="--",
            linewidth=1.8,
            color=color,
            label="_nolegend_",
        )

    ax.set_xlabel("Number of vertices ($n$)")
    ax.set_ylabel(ylabel)
    ax.set_xticks(sorted({row.n for row in rows}))

    if log_scale:
        ax.set_yscale("log")

    ax.grid(True, which="both", linestyle=":", linewidth=0.6)
    add_compact_legend(ax, ks, colors)
    fig.tight_layout()

    fig.savefig(out_dir / filename, bbox_inches="tight")
    fig.savefig(out_dir / filename.replace(".pdf", ".png"), dpi=300, bbox_inches="tight")
    plt.close(fig)


def write_reduction_factors(rows: list[Row], out_dir: Path) -> None:
    cp_lookup = {(row.n, row.k): row for row in rows if row.solver == "CP"}
    sap_lookup = {(row.n, row.k): row for row in rows if row.solver == "SAP"}

    output_path = out_dir / "reduction_factors.csv"

    with output_path.open("w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)
        writer.writerow(
            [
                "n",
                "k",
                "cp_time",
                "sap_time",
                "time_ratio_cp_over_sap",
                "cp_nodes",
                "sap_nodes",
                "node_ratio_cp_over_sap",
                "cp_failures",
                "sap_failures",
                "failure_ratio_cp_over_sap",
                "cp_timeouts",
                "sap_timeouts",
            ]
        )

        for key in sorted(cp_lookup):
            if key not in sap_lookup:
                continue

            cp = cp_lookup[key]
            sap = sap_lookup[key]

            def ratio(a: float, b: float) -> float | str:
                if b == 0:
                    return ""
                return a / b

            writer.writerow(
                [
                    cp.n,
                    cp.k,
                    cp.time_avg,
                    sap.time_avg,
                    ratio(cp.time_avg, sap.time_avg),
                    cp.nodes_avg,
                    sap.nodes_avg,
                    ratio(cp.nodes_avg, sap.nodes_avg),
                    cp.fail_avg,
                    sap.fail_avg,
                    ratio(cp.fail_avg, sap.fail_avg),
                    cp.timeouts,
                    sap.timeouts,
                ]
            )


def write_sap_activity_table(rows: list[Row], out_dir: Path) -> None:
    sap_rows = sorted(
        [row for row in rows if row.solver == "SAP"],
        key=lambda row: (row.n, row.k),
    )

    csv_path = out_dir / "sap_activity_table.csv"
    tex_path = out_dir / "sap_activity_table.tex"

    with csv_path.open("w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)
        writer.writerow(["n", "k", "sap_checks_avg", "sap_prunings_avg", "sap_conflicts_avg"])
        for row in sap_rows:
            writer.writerow(
                [
                    row.n,
                    row.k,
                    f"{row.sap_avg:.2f}",
                    f"{row.sap_prune_avg:.2f}",
                    f"{row.sap_conf_avg:.2f}",
                ]
            )

    lines = [
        r"\begin{table}[t]",
        r"\centering",
        r"\caption{SAP activity statistics averaged over included runs.}",
        r"\label{tab:sap_activity}",
        r"\begin{tabular}{rrrrr}",
        r"\hline",
        r"$n$ & $k$ & SAP checks & Prunings & Conflicts \\",
        r"\hline",
    ]

    for row in sap_rows:
        lines.append(
            f"{row.n} & {row.k} & {row.sap_avg:.2f} & "
            f"{row.sap_prune_avg:.2f} & {row.sap_conf_avg:.2f} \\\\"
        )

    lines.extend(
        [
            r"\hline",
            r"\end{tabular}",
            r"\end{table}",
            "",
        ]
    )

    tex_path.write_text("\n".join(lines), encoding="utf-8")


def write_timeout_summary(rows: list[Row], out_dir: Path) -> None:
    totals = {
        solver: sum(row.timeouts for row in rows if row.solver == solver)
        for solver in sorted({row.solver for row in rows})
    }

    csv_path = out_dir / "timeout_summary.csv"
    tex_path = out_dir / "timeout_summary.tex"

    with csv_path.open("w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)
        writer.writerow(["solver", "total_timeouts"])
        for solver, count in totals.items():
            writer.writerow([solver, count])

    lines = [
        r"\begin{table}[t]",
        r"\centering",
        r"\caption{Total number of timeouts across all benchmark classes.}",
        r"\label{tab:timeouts}",
        r"\begin{tabular}{lr}",
        r"\hline",
        r"Propagator & Timeouts \\",
        r"\hline",
    ]

    for solver, count in totals.items():
        lines.append(f"{solver} & {count} \\\\")

    lines.extend(
        [
            r"\hline",
            r"\end{tabular}",
            r"\end{table}",
            "",
        ]
    )

    tex_path.write_text("\n".join(lines), encoding="utf-8")

def weighted_average_by_n(
    rows: list[Row],
    solver: str,
    metric: str,
) -> tuple[list[int], list[float]]:
    result = []

    for n in sorted({row.n for row in rows}):
        selected = [row for row in rows if row.solver == solver and row.n == n]

        total_weight = sum(row.included for row in selected)
        if total_weight == 0:
            continue

        value = sum(getattr(row, metric) * row.included for row in selected) / total_weight
        result.append((n, value))

    ns = [n for n, _ in result]
    values = [value for _, value in result]
    return ns, values


def plot_nogood_lengths_by_n(rows: list[Row], out_dir: Path) -> None:
    fig, ax = plt.subplots(figsize=(7.2, 4.2))

    cp_ns, cp_values = weighted_average_by_n(rows, "CP", "nogood_len_avg")
    sap_ns, sap_values = weighted_average_by_n(rows, "SAP", "nogood_len_avg")

    ax.plot(cp_ns, cp_values, marker="o", linestyle="-", linewidth=2, label="CP")
    ax.plot(sap_ns, sap_values, marker="o", linestyle="--", linewidth=2, label="SAP")

    ax.set_xlabel("Number of vertices ($n$)")
    ax.set_ylabel("Average learned nogood length")
    ax.set_xticks(sorted({row.n for row in rows}))
    ax.grid(True, which="both", linestyle=":", linewidth=0.6)
    ax.legend(frameon=False)

    fig.tight_layout()
    fig.savefig(out_dir / "nogood_length_by_n.pdf", bbox_inches="tight")
    fig.savefig(out_dir / "nogood_length_by_n.png", dpi=300, bbox_inches="tight")
    plt.close(fig)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("stats_file", type=Path, help="Text file containing the summary tables.")
    parser.add_argument("--out", type=Path, default=Path("figuressss"), help="Output directory.")
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    rows = parse_stats(args.stats_file)

    plot_metric_by_n(
        rows,
        args.out,
        metric="time_avg",
        ylabel="Average runtime (s)",
        filename="runtime_by_n.pdf",
        log_scale=True,
    )
    plot_metric_by_n(
        rows,
        args.out,
        metric="nodes_avg",
        ylabel="Average search nodes",
        filename="nodes_by_n.pdf",
        log_scale=True,
    )
    plot_metric_by_n(
        rows,
        args.out,
        metric="fail_avg",
        ylabel="Average failures",
        filename="failures_by_n.pdf",
        log_scale=True,
    )
    plot_nogood_lengths_by_n(rows, args.out)

    write_reduction_factors(rows, args.out)
    write_sap_activity_table(rows, args.out)
    write_timeout_summary(rows, args.out)

    print(f"Created plots and tables in {args.out}")


if __name__ == "__main__":
    main()
