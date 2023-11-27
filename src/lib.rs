#![forbid(unsafe_code)]

mod internals;

pub use internals::{
    // resolution functions
    solver::{
        optimize_minimal, optimize_newest, parallel_optimize_minimal, parallel_optimize_newest,
        simple_solve,
    },
    // type definitions
    types::{
        ConstraintSet, Package, PackageId, PackageVer, Plan, Range, Repository, Requirement,
        RequirementSet, ResolutionError, ResolutionResult, Vec1, Version,
    },
};

pub use intmap::IntMap;
