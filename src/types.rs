use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use thiserror::Error;

type Version = u64;
type PackageId = u64;
type Index = u64;

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
    pub mapping: HashMap<PackageId, Index>,
}

#[derive(Eq, PartialEq, Debug, Error)]
pub enum ResolutionError<T> {
    IllegalIndex { package: PackageId, index: Index },
}

impl<T: Display> Display for ResolutionError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IllegalIndex { package, index } => {
                f.write_fmt(format!(
                    "Package with Id {} has illegal index {}",
                    package, index
                ))?;
            }
        }
        Ok(())
    }
}
