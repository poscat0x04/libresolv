## main features

Ordered by priority (descending).

- [x] Dependency resolution
- [x] (minimal) Unsat core generation and processing
- [x] Optimizing dependency resolution (optimizing for newest versions of packages)
- [x] Optimizing dependency resolution (optimizing for minimal number of packages)
- [x] Parallel resolution
- [ ] Repository generation and serialization
- [ ] High-level API

## enhancements

- [ ] Full documentation coverage
- [ ] Full test coverage
- [ ] Set up CI
- [ ] Look into z3 parameters and tactics to optimize performance
- [ ] Expose C API

## blocked by upstream issues

- [ ] use optimizer's own unsat core generation capability (blocked on [z3#7018](https://github.com/Z3Prover/z3/issues/7018))
- [ ] switch to official z3-rs crate when a new version with pkgconfig support drops

## maybe?

- [ ] multiple backends (specifically cvc5)
- [ ] (minimal) unsat core [enumeration](https://microsoft.github.io/z3guide/programming/Example%20Programs/Cores%20and%20Satisfying%20Subsets/)
- [ ] smtlib2 code emission
