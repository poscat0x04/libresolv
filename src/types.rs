use snafu::{prelude::*, Backtrace};
use std::{fmt::Display, iter::Chain, slice, vec};

pub type Version = u64;
pub type PackageId = u32;
pub type Index = u32;
pub type Plan = Vec<(PackageId, Version)>;

// Version range
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Range {
    Interval { lower: Version, upper: Version },
    Point(Version),
    All,
}

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Range::Interval { lower, upper } => write!(f, "[{lower}, {upper}]"),
            Range::Point(v) => write!(f, "{{{v}}}"),
            Range::All => write!(f, "𝒰"),
        }
    }
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

impl<'a> IntoIterator for &'a RequirementSet {
    type Item = &'a Requirement;
    type IntoIter = Chain<slice::Iter<'a, Requirement>, slice::Iter<'a, Requirement>>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.dependencies)
            .into_iter()
            .chain((&self.conflicts).into_iter())
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Debug)]
pub struct PackageVer {
    pub requirements: RequirementSet,
}

impl PackageVer {
    pub fn deps(&self) -> impl Iterator<Item = &Requirement> {
        self.requirements.dependencies.iter()
    }

    pub fn antideps(&self) -> impl Iterator<Item = &Requirement> {
        self.requirements.conflicts.iter()
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Package {
    pub id: PackageId,
    pub versions: Vec<PackageVer>,
}

impl Package {
    pub fn newest_version_number(&self) -> Version {
        self.versions.len() as Version
    }

    pub fn newest_version(&self) -> &PackageVer {
        &self.versions[self.newest_version_number() as usize - 1]
    }
}

pub struct Repository {
    pub packages: Vec<Package>,
}

impl Repository {
    pub fn get_package(&self, id: PackageId) -> Option<&Package> {
        self.packages.get(id as usize)
    }

    pub fn get_package_unchecked(&self, id: PackageId) -> &Package {
        &self.packages[id as usize]
    }
}

#[derive(Debug, Snafu)]
pub enum ResolutionError {
    #[snafu(display("Illegal index: Index {index} is out of bound"))]
    IllegalIndex { index: Index, backtrace: Backtrace },
    #[snafu(display("Timeout during dependency resolution"))]
    TimeOut { backtrace: Backtrace },
}

pub enum ResolutionResult {
    Unsat,
    UnsatWithCore {
        package_reqs: Vec<(PackageId, RequirementSet)>,
        toplevel_reqs: RequirementSet,
    },
    Sat {
        plan: Plan,
    },
}

pub type Res = Result<ResolutionResult, ResolutionError>;
