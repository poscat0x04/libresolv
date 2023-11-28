use crate::{
    Package, PackageId, PackageVer, Range, Repository, Requirement, RequirementSet, Version,
};
use indexmap::IndexMap;
use rkyv::{Archive, Deserialize, Serialize};

use std::hash::Hash;
use std::ops::RangeBounds;

pub trait SetOf<T> {
    fn contains(&self, t: &T) -> bool;

    #[allow(clippy::collapsible_else_if)]
    fn to_ranges<V>(&self, map: &IndexMap<T, V>) -> Vec<Range> {
        let mut ranges = Vec::new();
        let mut containing = false;
        let mut low = 1;
        let mut high;

        for (i, k) in map.keys().enumerate() {
            if containing {
                if !self.contains(k) {
                    containing = false;
                    high = i as Version;
                    if high == low {
                        ranges.push(Range::point(low));
                    } else {
                        ranges.push(Range::interval_unchecked(low, high));
                    }
                }
            } else {
                if self.contains(k) {
                    containing = true;
                    low = i as Version + 1;
                }
            }
        }

        if containing {
            high = map.len() as Version;
            if high == low {
                ranges.push(Range::point(low));
            } else {
                ranges.push(Range::interval_unchecked(low, high));
            }
        }

        ranges
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct ViaRangeBound<T>(pub T);

impl<T: Ord, V: RangeBounds<T>> SetOf<T> for ViaRangeBound<V> {
    fn contains(&self, t: &T) -> bool {
        self.0.contains(t)
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Union<R>(pub Vec<R>);

impl<T, R: SetOf<T>> SetOf<T> for Union<R> {
    fn contains(&self, t: &T) -> bool {
        self.0.iter().any(|r| r.contains(t))
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Intersection<R>(pub Vec<R>);

impl<T, R: SetOf<T>> SetOf<T> for Intersection<R> {
    fn contains(&self, t: &T) -> bool {
        self.0.iter().all(|r| r.contains(t))
    }
}

#[repr(transparent)]
pub struct ViaFunPtr<T>(pub for<'a> fn(&'a T) -> bool);

impl<T> SetOf<T> for ViaFunPtr<T> {
    fn contains(&self, t: &T) -> bool {
        self.0(t)
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum RepositoryBuildError<K, V, R> {
    UnknownPackage {
        source: K,
        version: V,
        unknown: K,
    },
    IllformedRequirement {
        source: K,
        version: V,
        requirement: ERequirement<K, R>,
    },
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct ERepository<K, V, R> {
    packages: IndexMap<K, EPackage<K, V, R>>,
    spine: Repository,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct ERepositoryBuilder<K, V, R> {
    packages: IndexMap<K, EPackage<K, V, R>>,
}

impl<K, V, R> ERepositoryBuilder<K, V, R>
where
    K: Clone + Hash + Eq + PartialEq,
    V: Clone + Hash,
    R: SetOf<V>,
{
    pub fn build(
        Self { packages }: Self,
    ) -> Result<ERepository<K, V, R>, RepositoryBuildError<K, V, R>>
    where
        R: Clone,
    {
        let mut pkgs = Vec::with_capacity(packages.len());

        for (i, (name, package)) in packages.iter().enumerate() {
            let mut versions = Vec::with_capacity(package.versions.len());

            for (v, version) in &package.versions {
                versions.push(version.translate(&packages).map_err(|e| match e {
                    Ok(k) => RepositoryBuildError::UnknownPackage {
                        source: name.clone(),
                        version: v.clone(),
                        unknown: k.clone(),
                    },
                    Err(r) => RepositoryBuildError::IllformedRequirement {
                        source: name.clone(),
                        version: v.clone(),
                        requirement: r.clone(),
                    },
                })?);
            }

            let pkg = Package {
                id: i as u32,
                versions,
            };
            pkgs.push(pkg);
        }

        let spine = Repository { packages: pkgs };
        Ok(ERepository { packages, spine })
    }

    pub fn new() -> Self {
        ERepositoryBuilder {
            packages: IndexMap::new(),
        }
    }

    pub fn with_capacity(n: usize) -> Self {
        ERepositoryBuilder {
            packages: IndexMap::with_capacity(n),
        }
    }

    pub fn add_package(&mut self, package: EPackage<K, V, R>) -> bool {
        if !(self.packages.contains_key(&package.name)) {
            let _ = self.packages.insert(package.name.clone(), package);
            true
        } else {
            false
        }
    }
}

impl<K, V, R> Default for ERepositoryBuilder<K, V, R>
where
    K: Clone + Hash + Eq + PartialEq,
    V: Clone + Hash,
    R: SetOf<V>,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct EPackage<K, V, R> {
    name: K,
    versions: IndexMap<V, EVersion<K, V, R>>,
}

#[derive(Eq, PartialEq, Debug, Clone, Archive, Serialize, Deserialize)]
pub struct EPackageBuilder<K, V, R> {
    name: K,
    versions: Vec<EVersion<K, V, R>>,
}

impl<K, V, R> EPackageBuilder<K, V, R>
where
    V: Ord + Hash + Clone,
{
    pub fn new(name: K) -> Self {
        EPackageBuilder {
            name,
            versions: Vec::new(),
        }
    }

    pub fn with_capacity(name: K, n: usize) -> Self {
        EPackageBuilder {
            name,
            versions: Vec::with_capacity(n),
        }
    }

    pub fn add_version(&mut self, version: EVersion<K, V, R>) {
        self.versions.push(version)
    }

    pub fn build(mut self) -> EPackage<K, V, R> {
        let mut versions = IndexMap::with_capacity(self.versions.len());

        self.versions.sort_by(|a, b| a.version.cmp(&b.version));

        for version in self.versions {
            if !(versions.contains_key(&version.version)) {
                let _ = versions.insert(version.version.clone(), version);
            }
        }

        EPackage {
            name: self.name,
            versions,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Archive, Serialize, Deserialize)]
pub struct EVersion<K, V, R> {
    version: V,
    dependencies: Vec<ERequirement<K, R>>,
    conflicts: Vec<ERequirement<K, R>>,
}

impl<K, V, R> EVersion<K, V, R>
where
    R: SetOf<V>,
    K: Eq + Hash,
{
    pub fn new(version: V) -> Self {
        EVersion {
            version,
            dependencies: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    pub fn from(
        version: V,
        dependencies: Vec<ERequirement<K, R>>,
        conflicts: Vec<ERequirement<K, R>>,
    ) -> Self {
        EVersion {
            version,
            dependencies,
            conflicts,
        }
    }

    pub fn with_capacity(version: V, n: usize) -> Self {
        EVersion {
            version,
            dependencies: Vec::with_capacity(n),
            conflicts: Vec::with_capacity(n),
        }
    }

    pub fn add_dependency(&mut self, requirement: ERequirement<K, R>) {
        self.dependencies.push(requirement)
    }

    pub fn add_conflict(&mut self, requirement: ERequirement<K, R>) {
        self.conflicts.push(requirement)
    }

    fn translate(
        &self,
        map: &IndexMap<K, EPackage<K, V, R>>,
    ) -> Result<PackageVer, Result<&K, &ERequirement<K, R>>> {
        let mut dependencies = Vec::with_capacity(self.dependencies.len());
        let mut conflicts = Vec::with_capacity(self.conflicts.len());

        for dep in self.dependencies.iter() {
            dependencies.push(dep.translate(map)?)
        }

        for antidep in self.conflicts.iter() {
            conflicts.push(antidep.translate(map)?)
        }

        Ok(PackageVer {
            requirements: RequirementSet {
                dependencies,
                conflicts,
            },
        })
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Archive, Serialize, Deserialize)]
pub struct ERequirement<K, R> {
    package: K,
    versions: R,
}

impl<K, R> ERequirement<K, R>
where
    K: Eq + Hash,
{
    pub fn new(package: K, versions: R) -> Self {
        ERequirement { package, versions }
    }

    fn translate<V>(
        &self,
        map: &IndexMap<K, EPackage<K, V, R>>,
    ) -> Result<Requirement, Result<&K, &Self>>
    where
        R: SetOf<V>,
    {
        let (id, _, package) = map.get_full(&self.package).ok_or(Ok(&self.package))?;
        let ranges = self
            .versions
            .to_ranges(&package.versions)
            .try_into()
            .map_err(|_| Err(self))?;
        Ok(Requirement {
            package: id as PackageId,
            versions: ranges,
        })
    }
}
