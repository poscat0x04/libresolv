# Dev Notes

## Explored design space

1.  Parallelism. Couldn't get parallel solving working no matter what. I've tried all possible combinations of 1. setting `parallel.enable` in global params 2. setting `threads` and related params in the solver 3. specifying the `QF_LIA` logic

2.  Manual model enumeration. Meaningless without parallelism as it is so much slower than the built-in optimizer.

3.  Specifying logic for solvers. `QF_FD` doesn't seem to work even if the domain is finite since the theory contains inequalities. `QF_LIA` seems to be the perfect logic. Specifying `QF_LIA` can make solving faster by 2x on a relatively large synthetic problem.

    Unfortunately we can't specify logics in optimizers

## Current design

Currently I think we should provide two types of resolution.

The first being a general resolution function that uses the solver
and produces locally optimal (in the sense of versions being newest) solutions.
This is intended to be fast and should work in the case where the constaint
counts are very large. It does this by first producing a random solution and
then repeatedly blocking the current solution and any solutions that are
"smaller". Until we can't find any solutions.

The second is a resolution function that uses the optimizer to produce globally
optimal solutions. Since the optimizer is very slow for large constaint sets
this function is rather niche.
