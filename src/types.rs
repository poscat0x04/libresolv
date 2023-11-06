use snafu::{prelude::*, Backtrace};
use std::{fmt::Display, iter::Chain, slice, vec};

// We use (initial segments of) positive integers to represent versions since the
// set of known versions are necessarily finite and hence are orderisomorphic
// to some initial segments.
//
// Additionally we use 0 to represent the "uninstalled" state.
pub type Version = u64;
pub const VER_WIDTH: u32 = 64;

// We use u32 to represent package ids to simplify lookups. This means we
// need to ensure the packages in a specific repo are in the right order
pub type PackageId = u32;
pub type Index = u32;

// An installation/build plan
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

impl Range {
    pub fn point(v: Version) -> Self {
        Self::Point(v)
    }

    pub fn interval(lower: Version, upper: Version) -> Option<Self> {
        if lower < upper {
            Some(Self::Interval { lower, upper })
        } else {
            None
        }
    }

    pub fn interval_unchecked(lower: Version, upper: Version) -> Self {
        Self::Interval { lower, upper }
    }

    pub fn all() -> Self {
        Self::All
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Requirement {
    pub package: PackageId,
    pub versions: Vec<Range>,
}

impl Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.versions.iter();
        if let Some(r) = iter.next() {
            write!(f, "Ver({}) ∈ {r}", self.package)?;
            for r in iter {
                write!(f, " ∪ {r}")?;
            }
            Ok(())
        } else {
            write!(f, "Ver({}) = 0", self.package)
        }
    }
}

impl Requirement {
    pub fn new(package: PackageId, versions: Vec<Range>) -> Self {
        Self { package, versions }
    }

    pub fn single_version(package: PackageId, version: Version) -> Self {
        Self {
            package,
            versions: vec![Range::point(version)],
        }
    }

    pub fn range(package: PackageId, lower: Version, upper: Version) -> Option<Self> {
        let r = Range::interval(lower, upper)?;
        Some(Self {
            package,
            versions: vec![r],
        })
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct RequirementSet {
    pub dependencies: Vec<Requirement>,
    pub conflicts: Vec<Requirement>,
}

impl Default for RequirementSet {
    fn default() -> Self {
        Self {
            dependencies: Vec::new(),
            conflicts: Vec::new(),
        }
    }
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

impl RequirementSet {
    pub fn from_deps(deps: Vec<Requirement>) -> Self {
        Self {
            dependencies: deps,
            conflicts: Vec::new(),
        }
    }

    pub fn from_antideps(antideps: Vec<Requirement>) -> Self {
        Self {
            dependencies: Vec::new(),
            conflicts: antideps,
        }
    }

    pub fn add_deps(&mut self, mut deps: Vec<Requirement>) {
        self.dependencies.append(&mut deps);
    }

    pub fn add_antideps(&mut self, mut antideps: Vec<Requirement>) {
        self.conflicts.append(&mut antideps);
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

#[derive(Eq, PartialEq, Debug)]
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
