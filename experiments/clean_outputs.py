from pathlib import Path

RESULTS_ROOT = Path("results/strong-articulation-extra")
OUTPUT_ROOT = Path("results/strong-articulation")


def clean_file(input_path: Path, output_path: Path):
    lines = input_path.read_text(
        encoding="utf-8",
        errors="replace",
    ).splitlines()

    cleaned_lines = []

    for line in lines:
        stripped = line.strip()

        if (
            stripped.startswith("% Generated FlatZinc statistics:")
            or stripped.startswith("%%%mzn-stat:")
            or stripped.startswith("%%%mzn-stat-end")
            or stripped.startswith("x =")
            or stripped == "----------"
            or stripped == "=========="
            or stripped == "=====UNKNOWN====="
        ):
            cleaned_lines.append(line)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        "\n".join(cleaned_lines) + "\n",
        encoding="utf-8",
    )


def main():
    for input_path in RESULTS_ROOT.rglob("*.txt"):
        relative_path = input_path.relative_to(RESULTS_ROOT)
        output_path = OUTPUT_ROOT / relative_path

        clean_file(input_path, output_path)
        print(f"Cleaned {input_path} -> {output_path}")

    print("Done.")


if __name__ == "__main__":
    main()