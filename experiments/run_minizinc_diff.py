import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
TESTS_ROOT = ROOT / "tests"

MODEL_FILE = TESTS_ROOT / "models" / "circuit_model.mzn"
INSTANCE_DIR = TESTS_ROOT / "instances"
RESULTS_DIR = TESTS_ROOT / "results"

LOG_FILE = RESULTS_DIR / "circuit_diff_verification.log"
TIMEOUT_SEC = 1800

PUMPKIN_MSC = ROOT / "minizinc" / "pumpkin.msc"
LEFT_SOLVER = "pumpkin-circuit"
RIGHT_SOLVER = "gecode"

_log_handle = None


def log(msg: str = "") -> None:
    print(msg, flush=True)
    if _log_handle is not None:
        _log_handle.write(msg + "\n")
        _log_handle.flush()


def rel(path) -> str:
    path = Path(path)
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def classify(exit_code: int) -> str:
    if exit_code == 0:
        return "MATCH"
    if exit_code == 255:
        return "MISMATCH"
    if exit_code == 1:
        return "LEFT_CRASH"
    if exit_code == 2:
        return "RIGHT_CRASH"
    if exit_code == 3:
        return "BOTH_CRASH"
    if exit_code == 5:
        return "LEFT_TIMEOUT"
    if exit_code == 6:
        return "RIGHT_TIMEOUT"
    if exit_code == 7:
        return "BOTH_TIMEOUT"
    return "ERROR"


def check_paths_exist(instances) -> None:
    missing = []

    if not MODEL_FILE.exists():
        missing.append(rel(MODEL_FILE))

    if not INSTANCE_DIR.exists():
        missing.append(rel(INSTANCE_DIR))

    if not PUMPKIN_MSC.exists():
        missing.append(rel(PUMPKIN_MSC))

    for instance in instances:
        if not instance.exists():
            missing.append(rel(instance))

    if missing:
        print("ERROR: missing files:", file=sys.stderr)
        for path in missing:
            print(f"  {path}", file=sys.stderr)
        sys.exit(1)


def run_one(instance_file: Path):
    argv = [
        "minizinc-diff",
        "diff",
        "-t",
        str(TIMEOUT_SEC),
        rel(MODEL_FILE),
        rel(instance_file),
        LEFT_SOLVER,
        RIGHT_SOLVER,
    ]

    started = time.monotonic()

    try:
        proc = subprocess.run(
            argv,
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        stdout = proc.stdout
        stderr = proc.stderr
        exit_code = proc.returncode
    except FileNotFoundError:
        log("ERROR: `minizinc-diff` not found on PATH.")
        sys.exit(1)
    except Exception as e:
        stdout = ""
        stderr = f"{type(e).__name__}: {e}"
        exit_code = -1

    wall = time.monotonic() - started

    return {
        "cmd": " ".join(argv),
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
        "status": classify(exit_code),
        "wall": wall,
    }


def main() -> None:
    global _log_handle

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    _log_handle = LOG_FILE.open("a", encoding="utf-8")

    instances = sorted(INSTANCE_DIR.rglob("*.dzn"))

    if not instances:
        print(f"ERROR: no .dzn files found in {rel(INSTANCE_DIR)}", file=sys.stderr)
        sys.exit(1)

    check_paths_exist(instances)

    log("=" * 80)
    log("circuit minizinc-diff verification")
    log(f"Instances:   {len(instances)}")
    log(f"Model:       {rel(MODEL_FILE)}")
    log(f"Left solver: {LEFT_SOLVER}")
    log(f"Right solver:{RIGHT_SOLVER}")
    log(f"Timeout/run: {TIMEOUT_SEC}s")
    log(f"Log file:    {rel(LOG_FILE)}")
    log("")

    summary_rows = []

    for i, instance in enumerate(instances, start=1):
        log("=" * 80)
        log(f"[{i:>3}/{len(instances)}] {rel(instance)}")

        result = run_one(instance)

        log(f"$ {result['cmd']}")
        log("--- stdout ---")
        log(result["stdout"].rstrip() if result["stdout"] else "(empty)")
        log("--- stderr ---")
        log(result["stderr"].rstrip() if result["stderr"] else "(empty)")
        log(
            f"exit_code: {result['exit_code']}  "
            f"status: {result['status']}  "
            f"wall_time: {result['wall']:.1f}s"
        )
        log("")

        summary_rows.append((rel(instance), result["status"], result["wall"]))

    log("=" * 80)
    log("SUMMARY")

    for instance, status, wall in summary_rows:
        log(f"  {instance:<55} {status:<14} {wall:>7.1f}s")

    counts = {}
    for _, status, _ in summary_rows:
        counts[status] = counts.get(status, 0) + 1

    breakdown = ", ".join(
        f"{count} {status}" for status, count in sorted(counts.items())
    )
    log(f"{len(summary_rows)} total: {breakdown}")

    _log_handle.close()

    bad_statuses = {
        "MISMATCH",
        "LEFT_CRASH",
        "RIGHT_CRASH",
        "BOTH_CRASH",
        "LEFT_TIMEOUT",
        "RIGHT_TIMEOUT",
        "BOTH_TIMEOUT",
        "ERROR",
    }

    if any(status in bad_statuses for _, status, _ in summary_rows):
        sys.exit(1)


if __name__ == "__main__":
    main()