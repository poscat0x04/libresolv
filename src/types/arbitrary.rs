use std::{
    cmp::{max, min},
    collections::BTreeMap,
    ops::Deref,
    rc::Rc,
};

use crate::types::*;
use itertools::Itertools;
use proptest::{
    collection::{btree_set, vec},
    prelude::*,
};

impl Range {
    prop_compose! {
        /// A strategy that generates random intervals between `inner` and `outer`.
        ///
        /// The shrinking behavior is to shrink the interval towards `inner`
        pub fn shrinking_strategy_
            (inner: (Version, Version), outer: (Version, Version))
            (lower_diff in 0..=(inner.0 - outer.0), upper_diff in 0..=(outer.1 - inner.1)) -> (Version, Version) {
            assert!(
                outer.0 <= inner.0 && inner.0 <= inner.1 && inner.1 <= outer.1,
                "Invalid range: {inner:?} not in {outer:?}"
            );
            (inner.0 - lower_diff, inner.1 + upper_diff)
        }
    }

    prop_compose! {
        /// A strategy that generates random ranges between `inner` and `outer`.
        ///
        /// The shrinking behavior is to shrink the range towards `inner`
        pub fn shrinking_strategy
            (inner: (Version, Version), outer: (Version, Version))
            ((lower, upper) in Self::shrinking_strategy_(inner, outer)) -> Range {
            Self::interval_unchecked(lower, upper)
        }
    }

    /// A strategy that generates random ranges between `inner` and `outer` with
    /// the upper and lower bounds of the range having random variation of amplitude `amplitude`.
    ///
    /// The shrinking behavior is to shrink the range towards `inner``
    pub fn shrinking_with_perturbation(
        inner: (Version, Version),
        outer: (Version, Version),
        amplitude: u32,
    ) -> impl Strategy<Value = Range> {
        Self::shrinking_strategy_(inner, outer).prop_perturb(
            move |(mut lower, mut upper), mut g| {
                let amplitude = amplitude as i64;
                lower = lower.saturating_add_signed(g.gen_range(-amplitude..=amplitude));
                upper = upper.saturating_add_signed(g.gen_range(-amplitude..=amplitude));
                Self::interval_unchecked(min(lower, upper), max(lower, upper))
            },
        )
    }

    /// A strategy that generates random ranges between `[center, center]` and `[1, max_ver]` with
    /// the upper and lower bounds of the range having random variation of amplitude `amplitude`,
    /// if `amplitude` is not None.
    ///
    /// The shrinking behavior is to shrink the range towards `inner`
    pub fn shrinking_centered(
        center: Version,
        max_ver: Version,
        amplitude: Option<u32>,
    ) -> BoxedStrategy<Range> {
        match amplitude {
            Some(amplitude) if amplitude != 0 => {
                Self::shrinking_with_perturbation((center, center), (1, max_ver), amplitude).boxed()
            }
            _ => Self::shrinking_strategy((center, center), (1, max_ver)).boxed(),
        }
    }

    prop_compose! {
        /// A strategy that generates random intervals between `inner` and `outer`.
        ///
        /// The shrinking behavior is to expand the interval towards `outer`
        pub fn expanding_strategy_
            (inner: (Version, Version), outer: (Version, Version))
            (lower_diff in 0..=(inner.0 - outer.0), upper_diff in 0..=(outer.1 - inner.1)) -> (Version, Version) {
            assert!(
                outer.0 <= inner.0 && inner.0 <= inner.1 && inner.1 <= outer.1,
                "Invalid range: {inner:?} not in {outer:?}"
            );
            (outer.0 + lower_diff, outer.1 - upper_diff)
        }
    }

    prop_compose! {
        /// A strategy that generates random ranges between `inner` and `outer`.
        ///
        /// The shrinking behavior is to expand the range towards `outer`
        pub fn expanding_strategy
            (inner: (Version, Version), outer: (Version, Version))
            ((lower, upper) in Self::expanding_strategy_(inner, outer)) -> Range {
            Self::interval_unchecked(lower, upper)
        }
    }

    /// A strategy that generates random ranges between `inner` and `outer` with
    /// the upper and lower bounds of the range having random variation of amplitude `amplitude`.
    ///
    /// The shrinking behavior is to expand the range towards `outer`
    pub fn expanding_with_perturbation(
        inner: (Version, Version),
        outer: (Version, Version),
        amplitude: u32,
    ) -> impl Strategy<Value = Range> {
        Self::expanding_strategy_(inner, outer).prop_perturb(
            move |(mut lower, mut upper), mut g| {
                let amplitude = amplitude as i64;
                lower = lower.saturating_add_signed(g.gen_range(-amplitude..=amplitude));
                upper = upper.saturating_add_signed(g.gen_range(-amplitude..=amplitude));
                Self::interval_unchecked(min(lower, upper), max(lower, upper))
            },
        )
    }

    /// A strategy that generates random ranges between `[center, center]` and `[1, max_ver]` with
    /// the upper and lower bounds of the range having random variation of amplitude `amplitude`,
    /// if `amplitude` is not None.
    ///
    /// The shrinking behavior is to expand the range towards `outer`
    pub fn expanding_centered(
        center: Version,
        max_ver: Version,
        amplitude: Option<u32>,
    ) -> BoxedStrategy<Range> {
        match amplitude {
            Some(amplitude) if amplitude != 0 => {
                Self::expanding_with_perturbation((center, center), (1, max_ver), amplitude).boxed()
            }
            _ => Self::expanding_strategy((center, center), (1, max_ver)).boxed(),
        }
    }
}

impl Requirement {
    prop_compose! {
        /// A wrapper around [`Range::shrinking_strategy`].
        pub fn shrinking_strategy(
            package: PackageId,
            inner: (Version, Version),
            outer: (Version, Version),
        )(range in Range::shrinking_strategy(inner, outer)) -> Requirement {
            Requirement::new(package, vec1![range])
        }
    }

    prop_compose! {
        /// A wrapper around [`Range::shrinking_with_perturbation`].
        pub fn shrinking_with_perturbation(
            package: PackageId,
            inner: (Version, Version),
            outer: (Version, Version),
            amplitude: u32,
        )(range in Range::shrinking_with_perturbation(inner, outer, amplitude)) -> Requirement {
            Requirement::new(package, vec1![range])
        }
    }

    prop_compose! {
        /// A wrapper around [`Range::shrinking_centered`].
        pub fn shrinking_centered(
            package: PackageId,
            center: Version,
            max_ver: Version,
            amplitude: Option<u32>,
        )(range in Range::shrinking_centered(center, max_ver, amplitude)) -> Requirement {
            Requirement::new(package, vec1![range])
        }
    }

    prop_compose! {
        /// A wrapper around [`Range::expanding_strategy`].
        pub fn expanding_strategy(
            package: PackageId,
            inner: (Version, Version),
            outer: (Version, Version),
        )(range in Range::expanding_strategy(inner, outer)) -> Requirement {
            Requirement::new(package, vec1![range])
        }
    }

    prop_compose! {
        /// A wrapper around [`Range::expanding_with_perturbation`].
        pub fn expanding_with_perturbation(
            package: PackageId,
            inner: (Version, Version),
            outer: (Version, Version),
            amplitude: u32,
        )(range in Range::expanding_with_perturbation(inner, outer, amplitude)) -> Requirement {
            Requirement::new(package, vec1![range])
        }
    }

    prop_compose! {
        /// A wrapper around [`Range::expanding_centered`].
        pub fn expanding_centered(
            package: PackageId,
            center: Version,
            max_ver: Version,
            amplitude: Option<u32>,
        )(range in Range::expanding_centered(center, max_ver, amplitude)) -> Requirement {
            Requirement::new(package, vec1![range])
        }
    }
}

impl RequirementSet {
    /// A strategy that generates `RequirementSet`s with random number of dependencies and conflicts
    /// (not exceeding half of the total number of packages). Each dependency and conflict is also
    /// randomly generated using [`Range::shrinking_centered`] and respects `max_versions`.
    pub fn random_reqset(
        max_versions: impl Deref<Target = Vec<Version>>,
        id: PackageId,
    ) -> impl Strategy<Value = RequirementSet> {
        let max_pid = max_versions.len() - 1;
        (
            btree_set(0..=max_pid, 0..=max_versions.len() / 2),
            btree_set(0..=max_pid, 0..=max_versions.len() / 2),
        )
            .prop_flat_map(move |(set1, set2)| {
                let conflicts = set2.difference(&set1);
                let dependency_strategies = set1
                    .iter()
                    .filter(|&&pid| pid as u32 != id)
                    .map(|&pid| {
                        let max_ver = max_versions[pid];
                        let center = (max_ver / 2) + 1;
                        Requirement::shrinking_centered(pid as u32, center, max_ver, None)
                    })
                    .collect_vec();
                let conflict_strategies = conflicts
                    .filter(|&&pid| pid as u32 != id)
                    .map(|&pid| {
                        let max_ver = max_versions[pid];
                        let center = (max_ver / 2) + 1;
                        Requirement::shrinking_centered(pid as u32, center, max_ver, None)
                    })
                    .collect_vec();
                (dependency_strategies, conflict_strategies).prop_map(
                    |(dependencies, conflicts)| RequirementSet {
                        dependencies,
                        conflicts,
                    },
                )
            })
    }

    /// A strategy that generates `RequirementSet`s that are guaranteed to have not cause conflicts
    /// with `required_installs`. Currently it does not generate any conflicts and each dependency
    /// generated is a a package that is required to be installed and the desired installation version
    /// is guaranteed to be contained in the generated range. If an `amplitude` is specified, the range
    /// will have random variation and as such the guarantee is no longer there.
    pub fn reqset_no_conflict(
        max_versions: impl Deref<Target = Vec<Version>>,
        required_installs: impl Deref<Target = BTreeMap<PackageId, Version>>,
        id: PackageId,
        amplitude: Option<u32>,
    ) -> impl Strategy<Value = RequirementSet> {
        let max_pid = max_versions.len() - 1;
        btree_set(0..=max_pid, 0..=max_versions.len() / 2).prop_flat_map(move |set| {
            let dependency_strategies = set
                .into_iter()
                .filter(|&pid| required_installs.contains_key(&(pid as u32)))
                .filter(|&pid| pid as u32 != id)
                .map(|pid| {
                    let max_ver = max_versions[pid];
                    let center = required_installs
                        .get(&(pid as u32))
                        .expect("Impossible: pid not in required_installs despite being filtered");
                    assert!(
                        *center <= max_ver,
                        "Invalid required_installs: center > max_ver"
                    );
                    Requirement::shrinking_centered(pid as u32, *center, max_ver, amplitude)
                })
                .collect_vec();
            dependency_strategies.prop_map(|dependencies| RequirementSet {
                dependencies,
                conflicts: vec![],
            })
        })
    }
}

impl PackageVer {
    prop_compose! {
        /// A wrapper around [`RequirementSet::random_reqset`]
        pub fn random_pkgver(
            max_versions: impl Deref<Target = Vec<Version>>,
            id: PackageId,
        )(requirements in RequirementSet::random_reqset(max_versions, id)) -> PackageVer {
            PackageVer { requirements }
        }
    }

    prop_compose! {
        /// A wrapper around [`RequirementSet::reqset_no_conflict`]
        pub fn pkgver_respecting_req_installs(
            max_versions: impl Deref<Target = Vec<Version>>,
            required_installs: impl Deref<Target = BTreeMap<PackageId, Version>>,
            id: PackageId,
            amplitude: Option<u32>,
        )(requirements in RequirementSet::reqset_no_conflict(
            max_versions,
            required_installs,
            id,
            amplitude,
        )) -> PackageVer {
            PackageVer { requirements }
        }
    }
}

impl Package {
    /// A strategy that generates completely random packages with package id `id`
    /// and with total number of versions matching the supplied `max_versions`.
    ///
    /// Each version is also random and have dependencies/conflicts
    /// that respect `max_versions`
    pub fn random_package(
        max_versions: Rc<Vec<Version>>,
        id: PackageId,
    ) -> impl Strategy<Value = Package> {
        assert!(
            (id as usize) < max_versions.len(),
            "Invalid package id or max_versions: index out of bound"
        );
        let max_ver = max_versions[id as usize];
        assert!(
            max_ver != 0,
            "Invalid max_ver: max_ver should be greater than 0"
        );
        let ver_strategies = (1..=max_ver)
            .map(|_version| PackageVer::random_pkgver(max_versions.clone(), id))
            .collect_vec();
        ver_strategies.prop_map(move |versions| Package { id, versions })
    }

    /// A strategy that generates random packages with package id `id` and
    /// with total number of versions matching the supplied `max_versions`.
    ///
    /// If the package is required to be installed, then a neighborhood of versions
    /// around the required installed version will be guaranteed to not cause conflicts
    /// with `required_installs`, unless of course, if an `amplitude` is specified.
    pub fn pkg_respecting_req_installs(
        max_versions: Rc<Vec<Version>>,
        id: PackageId,
        required_installs: Rc<BTreeMap<PackageId, Version>>,
        amplitude: Option<u32>,
    ) -> BoxedStrategy<Package> {
        if required_installs.contains_key(&id) {
            assert!(
                (id as usize) < max_versions.len(),
                "Invalid package id or max_versions: index out of bound"
            );
            let max_ver = max_versions[id as usize];
            assert!(
                max_ver != 0,
                "Invalid max_ver: max_ver should be greater than 0"
            );
            let center = required_installs[&id];
            (0..=5u64)
                .prop_flat_map(move |n| {
                    let ver_strategies = (1..=max_ver)
                        .map(|version| {
                            if center.abs_diff(version) <= n {
                                PackageVer::pkgver_respecting_req_installs(
                                    max_versions.clone(),
                                    required_installs.clone(),
                                    id,
                                    amplitude,
                                )
                                .boxed()
                            } else {
                                PackageVer::random_pkgver(max_versions.clone(), id).boxed()
                            }
                        })
                        .collect_vec();
                    ver_strategies.prop_map(move |versions| Package { id, versions })
                })
                .boxed()
        } else {
            Package::random_package(max_versions, id).boxed()
        }
    }
}

impl Repository {
    pub fn random_repo_with_size(
        pkg_count: usize,
        installed_pkg_count: usize,
        max_ver: Version,
        amplitude: Option<u32>,
    ) -> impl Strategy<Value = (Repository, Rc<BTreeMap<PackageId, Version>>)> {
        (
            vec(1..=max_ver, pkg_count),
            btree_set(0..=pkg_count - 1, installed_pkg_count),
        )
            .prop_flat_map(move |(max_versions, required_packages)| {
                let max_versions = Rc::new(max_versions);
                let required_version_strategies = required_packages
                    .iter()
                    .map(|&pid| 1..=max_versions[pid])
                    .collect_vec();
                required_version_strategies.prop_flat_map(move |required_pkg_versions| {
                    let required_installs: Rc<BTreeMap<u32, u64>> = Rc::new(
                        required_packages
                            .clone()
                            .into_iter()
                            .map(|pid| pid as PackageId)
                            .zip(required_pkg_versions)
                            .collect(),
                    );
                    let pkg_strategies = (0..=pkg_count - 1)
                        .map(|pid| {
                            Package::pkg_respecting_req_installs(
                                max_versions.clone(),
                                pid as PackageId,
                                required_installs.clone(),
                                amplitude,
                            )
                        })
                        .collect_vec();
                    pkg_strategies.prop_map(move |packages| {
                        (Repository { packages }, required_installs.clone())
                    })
                })
            })
    }
}

#[cfg(test)]
mod test {
    use pretty::Arena;
    use proptest::prelude::*;
    use termcolor::{ColorChoice, StandardStream};

    use crate::{solver::optimize_newest, types::*};

    proptest! {
        #![proptest_config(ProptestConfig {
            fork: false,
            .. ProptestConfig::default()
        })]
        #[test]
        fn test_parallel_solver(
            (repo, required_installs) in Repository::random_repo_with_size(10, 3, 15, None)
        ) {
            let arena = Arena::new();
            let stdout = StandardStream::stdout(ColorChoice::Auto);
            let doc = repo.clone().pretty(&arena);
            let _ = doc.render_colored(80, stdout);
            let dependencies =
                required_installs
                 .iter()
                 .map(|(&pid, _)| Requirement { package: pid, versions: vec1![Range::all()]})
                 .collect_vec();
            let requirements = RequirementSet { dependencies, conflicts: vec![] };
            let result = optimize_newest(&repo, &requirements).unwrap();
            println!("{result:?}");
            prop_assert!(result.is_sat())
        }
    }
}
