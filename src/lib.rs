#![forbid(unsafe_code)]
#[macro_use]
extern crate vec1;

mod internals;

pub use internals::{
    // resolution functions
    solver::{optimize_minimal, optimize_newest, simple_solve},
    // type definitions
    types::{
        ConstraintSet, Package, PackageId, PackageVer, Plan, Range, Repository, Requirement,
        RequirementSet, ResolutionError, ResolutionResult, Version,
    },
};

pub use intmap::IntMap;
pub use vec1::Vec1;
