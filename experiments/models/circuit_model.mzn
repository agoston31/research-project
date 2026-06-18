include "globals.mzn";

int: n;
array[1..n] of var 1..n: x;

array[1..n] of set of int: allowed;

% Nodes can only be assigned a successor if there is an edge going there
constraint forall(i in 1..n)(
    x[i] in allowed[i]
);

constraint circuit(x);

solve
:: int_search(x, input_order, indomain_min, complete)
satisfy;