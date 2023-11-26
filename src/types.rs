#[cfg(feature = "arbitrary")]
pub(crate) mod arbitrary;
pub(crate) mod expr;

use intmap::IntMap;
use itertools::Itertools;
use pretty::{DocAllocator, DocBuilder, Pretty};
use std::{cmp::Ordering, fmt::Display, iter::Chain, slice, vec};
use termcolor::ColorSpec;
use vec1::Vec1;

use crate::utils::{blue_text, green_text, red_text};

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

impl<'a, D, A> Pretty<'a, D, A> for Range
where
    D: DocAllocator<'a, A>,
    A: 'a,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        allocator.as_string(self)
    }
}

impl Range {
    pub fn point(v: Version) -> Self {
        Self::Point(v)
    }

    pub fn interval(lower: Version, upper: Version) -> Option<Self> {
        match lower.cmp(&upper) {
            Ordering::Less => Some(Self::Interval { lower, upper }),
            Ordering::Equal => Some(Self::Point(lower)),
            Ordering::Greater => None,
        }
    }

    pub fn interval_unchecked(lower: Version, upper: Version) -> Self {
        Self::Interval { lower, upper }
    }

    pub fn all() -> Self {
        Self::All
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Requirement {
    pub package: PackageId,
    pub versions: Vec1<Range>,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for Requirement
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        RequirementPretty {
            req: self,
            invert: false,
        }
        .pretty(allocator)
    }
}

#[derive(Eq, PartialEq, Debug)]
struct RequirementPretty {
    req: Requirement,
    invert: bool,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for RequirementPretty
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        allocator
            .text(format!("Ver({})", self.req.package))
            .annotate(blue_text())
            + allocator.space()
            + if self.invert {
                allocator.text("∉").annotate(red_text())
            } else {
                allocator.text("∈").annotate(green_text())
            }
            + allocator.space()
            + allocator
                .intersperse(self.req.versions, allocator.text(" ∪") + allocator.line())
                .align()
                .group()
    }
}

impl Requirement {
    pub fn new(package: PackageId, versions: Vec1<Range>) -> Self {
        Self { package, versions }
    }

    pub fn any_version(package: PackageId) -> Self {
        Self {
            package,
            versions: vec1![Range::all()],
        }
    }

    pub fn single_version(package: PackageId, version: Version) -> Self {
        Self {
            package,
            versions: vec1![Range::point(version)],
        }
    }

    pub fn range(package: PackageId, lower: Version, upper: Version) -> Option<Self> {
        let r = Range::interval(lower, upper)?;
        Some(Self {
            package,
            versions: vec1![r],
        })
    }
}

#[derive(Eq, PartialEq, Debug, Default, Clone)]
pub struct RequirementSet {
    pub dependencies: Vec<Requirement>,
    pub conflicts: Vec<Requirement>,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for RequirementSet
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        (allocator.intersperse(self.dependencies, allocator.hardline())
            + allocator.hardline()
            + allocator.intersperse(
                self.conflicts
                    .into_iter()
                    .map(|req| RequirementPretty { req, invert: true }),
                allocator.hardline(),
            ))
        .align()
    }
}

impl IntoIterator for RequirementSet {
    type Item = Requirement;
    type IntoIter = Chain<vec::IntoIter<Self::Item>, vec::IntoIter<Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        self.dependencies.into_iter().chain(self.conflicts)
    }
}

impl<'a> IntoIterator for &'a RequirementSet {
    type Item = &'a Requirement;
    type IntoIter = Chain<slice::Iter<'a, Requirement>, slice::Iter<'a, Requirement>>;

    fn into_iter(self) -> Self::IntoIter {
        self.dependencies.iter().chain(&self.conflicts)
    }
}

impl RequirementSet {
    pub fn from_dep(dep: Requirement) -> Self {
        Self {
            dependencies: vec![dep],
            conflicts: Vec::new(),
        }
    }

    pub fn from_deps(deps: Vec<Requirement>) -> Self {
        Self {
            dependencies: deps,
            conflicts: Vec::new(),
        }
    }

    pub fn from_antidep(antidep: Requirement) -> Self {
        Self {
            dependencies: Vec::new(),
            conflicts: vec![antidep],
        }
    }

    pub fn from_antideps(antideps: Vec<Requirement>) -> Self {
        Self {
            dependencies: Vec::new(),
            conflicts: antideps,
        }
    }

    pub fn add_dep(&mut self, dep: Requirement) {
        self.dependencies.push(dep);
    }

    pub fn add_deps(&mut self, mut deps: Vec<Requirement>) {
        self.dependencies.append(&mut deps);
    }

    pub fn add_antidep(&mut self, antidep: Requirement) {
        self.conflicts.push(antidep);
    }

    pub fn add_antideps(&mut self, mut antideps: Vec<Requirement>) {
        self.conflicts.append(&mut antideps);
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct PackageVer {
    pub requirements: RequirementSet,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for PackageVer
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        self.requirements.pretty(allocator)
    }
}

#[cfg(feature = "arbitrary")]

impl PackageVer {
    pub fn deps(&self) -> impl Iterator<Item = &Requirement> {
        self.requirements.dependencies.iter()
    }

    pub fn antideps(&self) -> impl Iterator<Item = &Requirement> {
        self.requirements.conflicts.iter()
    }
}

struct PackageVerPretty {
    reqs: RequirementSet,
    ver_number: Version,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for PackageVerPretty
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        (allocator.text(format!("Ver = {} ⇒", self.ver_number))
            + allocator.hardline()
            + self.reqs.pretty(allocator).indent(2))
        .align()
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Package {
    pub id: PackageId,
    pub versions: Vec<PackageVer>,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for Package
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        (allocator.text(format!("Package({}):", self.id))
            + allocator.hardline()
            + allocator
                .intersperse(
                    self.versions
                        .into_iter()
                        .zip(1..)
                        .map(|(ver, ver_number)| PackageVerPretty {
                            reqs: ver.requirements,
                            ver_number,
                        }),
                    allocator.hardline(),
                )
                .align()
                .indent(2))
        .align()
    }
}

impl Package {
    pub fn newest_version_number(&self) -> Version {
        self.versions.len() as Version
    }

    pub fn newest_version(&self) -> &PackageVer {
        &self.versions[self.newest_version_number() as usize - 1]
    }
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Repository {
    pub packages: Vec<Package>,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for Repository
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        allocator
            .intersperse(self.packages, allocator.hardline())
            .align()
    }
}

impl Repository {
    pub fn get_package(&self, id: PackageId) -> Option<&Package> {
        self.packages.get(id as usize)
    }

    pub fn get_package_unchecked(&self, id: PackageId) -> &Package {
        &self.packages[id as usize]
    }

    pub fn newest_ver_of(&self, id: PackageId) -> Option<Version> {
        self.get_package(id).map(|p| p.newest_version_number())
    }

    pub fn newest_ver_of_unchecked(&self, id: PackageId) -> Version {
        self.get_package_unchecked(id).newest_version_number()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ResolutionError {
    TimeOut,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct ConstraintSet {
    pub package_reqs: IntMap<IntMap<RequirementSet>>,
    pub toplevel_reqs: RequirementSet,
}

impl<'a, D> Pretty<'a, D, ColorSpec> for ConstraintSet
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        let pkg_constraint_doc = {
            let mut doc = allocator.nil();
            let mut pkg_reqs = self.package_reqs.into_iter().collect_vec();
            pkg_reqs.sort_by_key(|(pid, _)| *pid);
            for (pid, reqs) in pkg_reqs {
                let mut reqs = reqs.into_iter().collect_vec();
                reqs.sort_by_key(|(version, _)| *version);
                doc += allocator.text(format!("Package {pid}:"))
                    + allocator.hardline()
                    + allocator
                        .intersperse(
                            reqs.into_iter()
                                .map(|(ver_number, req_set)| PackageVerPretty {
                                    reqs: req_set,
                                    ver_number,
                                }),
                            allocator.hardline(),
                        )
                        .align()
                        .indent(2)
            }
            doc = doc.align();
            doc
        };
        allocator.text("Top-level constraints:")
            + self.toplevel_reqs.pretty(allocator).indent(2)
            + allocator.hardline()
            + allocator.text("Package constraints:")
            + allocator.hardline()
            + pkg_constraint_doc.indent(2)
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ResolutionResult {
    Unsat,
    UnsatWithCore { core: ConstraintSet },
    Sat { plans: Vec1<Plan> },
}

impl<'a, D> Pretty<'a, D, ColorSpec> for ResolutionResult
where
    D: DocAllocator<'a, ColorSpec>,
    D::Doc: Clone,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, ColorSpec> {
        match self {
            Self::Unsat => allocator.text("Unsat"),
            Self::UnsatWithCore { core } => {
                allocator.text("Unsat, minimal unsatisifable core:")
                    + allocator.hardline()
                    + core.pretty(allocator)
            }
            Self::Sat { plans } => {
                let mut doc = allocator
                    .text("Satisifiable with the following (optimal) installation plan(s):")
                    + allocator.hardline();
                for (mut plan, index) in plans.into_iter().zip(1..) {
                    plan.sort_by_key(|(pid, _)| *pid);
                    doc += allocator.text(format!("{index}.")) + allocator.hardline();
                    doc += allocator
                        .intersperse(
                            plan.into_iter().map(|(pid, version)| {
                                allocator.text(format!("Ver({pid}) = {version}"))
                            }),
                            allocator.hardline(),
                        )
                        .align()
                        .indent(2);
                }
                doc
            }
        }
    }
}

impl ResolutionResult {
    pub fn is_sat(&self) -> bool {
        matches!(self, Self::Sat { .. })
    }

    pub fn is_unsat(&self) -> bool {
        !self.is_sat()
    }
}

pub type Res = Result<ResolutionResult, ResolutionError>;

#[cfg(test)]
mod test {
    use crate::types::Requirement;

    use super::{Range, RequirementSet};
    use pretty::{Arena, Pretty};
    use termcolor::{ColorChoice, StandardStream};

    #[test]
    fn test_version_pretty() {
        let arena = Arena::new();
        let req = Requirement {
            package: 1,
            versions: vec1![
                Range::interval_unchecked(1, 2),
                Range::interval_unchecked(3, 4),
                Range::interval_unchecked(5, 6),
            ],
        };
        let reqs = RequirementSet::from_antidep(req);
        let doc = reqs.pretty(&arena);
        let stdout = StandardStream::stdout(ColorChoice::Auto);
        doc.render_colored(20, stdout).unwrap()
    }
}
