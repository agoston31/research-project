import subprocess
import os

# Create a folder to group the .dzn and .fzn file
def create_instance_folder(n, k, seed):
    folder = f"instances/n{n}_k{k}_seed{seed}"
    os.makedirs(folder, exist_ok=True)
    return folder

# Write the dzn file to the correct location
def write_dzn(edges, n, k, seed, folder):
    filename = f"{folder}/instance.dzn"

    with open(filename, "w") as f:
        f.write(f"n = {n};\n")

        f.write("allowed = [\n")
        for i in range(n):
            row = sorted(edges[i])
            # Convert to 1-based indexing
            row_str = ", ".join(str(j+1) for j in row)

            f.write(f"    [{row_str}]")
            if i < n - 1:
                f.write(",\n")
            else:
                f.write("\n")
        f.write("];\n")

    return filename

# Compile the fzn file and put it in the right location with the -o flag
# Prevent ozn file from generating
def compile_fzn(model_file, dzn_file, folder):
    fzn_file = os.path.join(folder, "instance.fzn")

    cmd = [
        "minizinc",
        "--solver", "pumpkin",
        "--compile",
        "--no-output-ozn",
        "-o", fzn_file,
        model_file,
        dzn_file
    ]

    result = subprocess.run(cmd, capture_output=True, text=True)


    if result.returncode != 0:
        print("Error compiling")
        print(result.stderr)
    else:
        print(f"Compiled successfully: {fzn_file}")

    return fzn_file