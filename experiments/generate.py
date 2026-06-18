from generator import generate_instance
from minizinc_io import write_dzn, compile_fzn, create_instance_folder
import random
import sys

def generate_and_save(n, k, seed=None, model_file="models/circuit_model.mzn"):
    if seed is None:
        seed = random.randint(1, 1* 10**6)

    # Create folder
    folder = create_instance_folder(n, k, seed)

    # Generate graph
    edges, order = generate_instance(n, k, seed)

    # Write instance
    dzn_file = write_dzn(edges, n, k, seed, folder)

    # Compile to FlatZinc
    fzn_file = compile_fzn(model_file, dzn_file, folder)

    return folder, dzn_file, fzn_file

# Logic for inputting parameters for test generation
if __name__ == "__main__":
    if len(sys.argv) == 4:
        n = int(sys.argv[1])
        k = int(sys.argv[2])
        if(k > n):
            print("k will be restricted to k < n")
        seed = int(sys.argv[3])
        generate_and_save(n, k, seed)
    elif len(sys.argv) == 3:
        n = int(sys.argv[1])
        k = int(sys.argv[2])
        if(k > n):
            print("k will be restricted to k < n")
        generate_and_save(n, k)
    else:
        print("Usage: python generate.py n k (optional: seed)")