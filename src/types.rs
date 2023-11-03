use snafu::{prelude::*, Backtrace};
use std::{iter::Chain, vec};

pub type Version = u64;
pub type PackageId = u64;
pub type Index = u64;

// Version range
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Range {
    Interval { lower: Version, upper: Version },
    Point(Version),
    All,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Requirement {
    pub package: PackageId,
    pub versions: Vec<Range>,
}

#[derive(Eq, PartialEq, Debug)]
pub struct RequirementSet {
    pub dependencies: Vec<Requirement>,
    pub conflicts: Vec<Requirement>,
}

impl IntoIterator for RequirementSet {
    type Item = Requirement;
    type IntoIter = Chain<vec::IntoIter<Self::Item>, vec::IntoIter<Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        self.dependencies
            .into_iter()
            .chain(self.conflicts.into_iter())
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Debug)]
pub struct PackageVer {
    pub requirements: RequirementSet,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Package {
    pub id: PackageId,
    pub versions: Vec<PackageVer>,
}

pub struct Repository {
    pub packages: Vec<Package>,
}

#[derive(Debug, Snafu)]
pub enum ResolutionError {
    #[snafu(display("Illegal index: Index {index} is out of bound"))]
    IllegalIndex { index: Index, backtrace: Backtrace },
}
