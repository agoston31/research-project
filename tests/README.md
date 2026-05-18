<h1>How to generate test cases</h1>

``python generate.py n k (seed)``

- ``n`` defines the amount of nodes for this problem instance
- ``k`` defines the amount of edges every node will have to their closest neighbours before creating extra ones to ensure a Hamiltonian cycle
- ``seed`` can be given to generate a specific test. If left out, a random seed will be generated.

``generate.py`` generates a single test instance with the given parameters in .dzn format and automatically converts it to FlatZinc using the Pumpkin solver. Both generated files can be found under the ``instances`` folder. The name of the subfolder specifies ``n``, ``k`` and the ``seed``.

<h1>How to run tests</h1>

After creating a .fzn file, it can be run using the following command:
```
cargo run -p pumpkin-solver instances/[folder_name]/instance.fzn
```
