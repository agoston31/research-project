include "globals.mzn";

int: n;
array[1..n] of var 1..n: x;

array[1..n] of array[int] of int: allowed;

% Nodes can only be assigned a successor if there is an edge going there
constraint forall(i in 1..n)(
    exists(j in 1..length(allowed[i])) (
        x[i] = allowed[i][j]
    )
);

constraint circuit(x);

solve satisfy;