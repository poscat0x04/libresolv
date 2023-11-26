use crate::{
    constraints::{add_all_constraints, find_closure},
    types::{
        expr::{AtomicExpr, Expr},
        *,
    },
    utils::iter_max_map,
    z3_helpers::{
        default_config, default_params, distance_from_newest, enumerate_models,
        eval_int_expr_in_model, installed_packages,
    },
};

use bumpalo::Bump;
use intmap::IntMap;
use itertools::Itertools;
use std::collections::HashMap;
use tinyset::SetU32;
use vec1::Vec1;
use z3::{
    ast::{Ast, Bool, Int},
    Context, Model, Optimize, Solver,
};

fn plan_from_model(ctx: &Context, model: Model, pids: impl Iterator<Item = PackageId>) -> Plan {
    let mut plan = Vec::new();
    let mut no_interp = Vec::new();
    let mut interp_not_u64 = Vec::new();

    for package_id in pids {
        let p = Int::new_const(ctx, package_id);
        if let Some(interp) = model.get_const_interp(&p) {
            if let Some(v) = interp.as_u64() {
                plan.push((package_id, v))
            } else {
                interp_not_u64.push(package_id);
            }
        } else {
            no_interp.push(package_id);
        }
    }

    if !(no_interp.is_empty() && interp_not_u64.is_empty()) {
        let mut panic_msg = String::new();

        panic_msg.push_str("Impossible: failed to generate a plan from a model, reasons:\n");

        if !no_interp.is_empty() {
            panic_msg
                .push_str("The following packages do not have an interpretation in the model:\n");
            panic_msg.push_str(&format!("  {no_interp:?}"))
        }

        if !interp_not_u64.is_empty() {
            panic_msg.push_str(
                "The following packages have an interpretation but the value cannot fit in a u64:\n",
            );
            panic_msg.push_str(&format!("  {interp_not_u64:?}"))
        }
        panic!("{panic_msg}");
    }
    plan
}

fn process_unsat_core(repo: &Repository, core_assertions: Vec<&Expr<'_>>) -> ConstraintSet {
    let mut package_reqs: IntMap<IntMap<RequirementSet>> = IntMap::new();
    let mut dependencies = Vec::new();
    let mut conflicts = Vec::new();
    for assertion in core_assertions {
        match assertion {
            Expr::Atom(e) => match e {
                AtomicExpr::VerEq { pid, version } => {
                    if *version == 0 {
                        conflicts.push(Requirement::new(*pid, vec1![Range::all()]))
                    } else {
                        dependencies.push(Requirement::new(*pid, vec1![Range::point(*version)]))
                    }
                }
                AtomicExpr::VerLE { pid, version } => {
                    if *version != repo.newest_ver_of_unchecked(*pid) {
                        panic!("Assertion {assertion} does not have a matching lower bound, this should not be possible")
                    }
                }
                AtomicExpr::VerGE { pid: _, version } => {
                    if *version != 0 {
                        panic!("Assertion {assertion} does not have a matching upper bound, this should not be possible")
                    }
                }
            },
            Expr::Not(e) => {
                let req = process_version_range(e);
                conflicts.push(req);
            }
            Expr::Implies(Expr::Atom(AtomicExpr::VerEq { pid, version }), rhs) => {
                let req;
                let mut reverse = false;
                match rhs {
                    Expr::Atom(AtomicExpr::VerEq {
                        pid: pid2,
                        version: 0,
                    }) => {
                        req = Some(Requirement::new(*pid2, vec1![Range::all()]));
                        reverse = true;
                    }
                    Expr::Not(e) => {
                        req = Some(process_version_range(e));
                        reverse = true;
                    }
                    _ => {
                        req = Some(process_version_range(rhs));
                    }
                }
                let req_ = req.unwrap();

                if let Some(ver_req_map) = package_reqs.get_mut(*pid as u64) {
                    if let Some(req_set) = ver_req_map.get_mut(*version) {
                        if reverse {
                            req_set.add_antidep(req_)
                        } else {
                            req_set.add_dep(req_)
                        }
                    } else {
                        let req_set = if reverse {
                            RequirementSet::from_antidep(req_)
                        } else {
                            RequirementSet::from_dep(req_)
                        };
                        ver_req_map.insert(*version, req_set);
                    }
                } else {
                    let mut ver_req_map = IntMap::new();
                    let req_set = if reverse {
                        RequirementSet::from_antidep(req_)
                    } else {
                        RequirementSet::from_dep(req_)
                    };
                    ver_req_map.insert(*version, req_set);
                    package_reqs.insert(*pid as u64, ver_req_map);
                }
            }
            _ => {
                let req = process_version_range(assertion);
                dependencies.push(req);
            }
        }
    }

    ConstraintSet {
        package_reqs,
        toplevel_reqs: RequirementSet {
            dependencies,
            conflicts,
        },
    }
}

fn process_version_range(expr: &Expr<'_>) -> Requirement {
    fn go(expr: &Expr<'_>) -> (PackageId, Vec1<Range>) {
        match expr {
            Expr::Atom(AtomicExpr::VerEq { pid, version }) => (*pid, vec1![Range::point(*version)]),
            Expr::And(lhs, rhs) => {
                let mut lb = 0;
                let mut ub = 0;
                let package_id;
                match lhs {
                    Expr::Atom(AtomicExpr::VerGE { pid, version }) => {
                        lb = *version;
                        package_id = *pid;
                    }
                    Expr::Atom(AtomicExpr::VerLE { pid, version }) => {
                        ub = *version;
                        package_id = *pid;
                    }
                    _ => panic!("Impossible: unknown lhs {lhs} of the expression {expr}"),
                }
                match rhs {
                    Expr::Atom(AtomicExpr::VerGE { pid, version }) => {
                        lb = *version;
                        assert_eq!(package_id, *pid);
                    }
                    Expr::Atom(AtomicExpr::VerLE { pid, version }) => {
                        ub = *version;
                        assert_eq!(package_id, *pid);
                    }
                    _ => panic!("Impossible: unknown rhs {rhs} of the expression {expr}"),
                }
                let rs = vec1![Range::interval(lb, ub).unwrap_or_else(|| {
                    panic!("Impossible: lower bound is bigger than upper bound in expr {expr}")
                })];
                (package_id, rs)
            }
            Expr::Or(lhs, rhs) => {
                let (pid1, mut rs1) = go(lhs);
                let (pid2, rs2) = go(rhs);
                assert_eq!(pid1, pid2);
                rs1.append(&mut rs2.into_vec());
                (pid1, rs1)
            }
            Expr::Not(Expr::Atom(AtomicExpr::VerEq { pid, version: 0 })) => {
                (*pid, vec1![Range::all()])
            }
            _ => panic!("Impossible: unknown expression {expr} for version range(s)"),
        }
    }

    let (pid, ranges) = go(expr);
    Requirement::new(pid, ranges)
}

pub fn simple_solve(repo: &Repository, requirements: &RequirementSet) -> Res {
    let cfg = default_config();
    let ctx = Context::new(&cfg);
    let solver = Solver::new_for_logic(&ctx, "QF_LIA").unwrap();
    solver.set_params(&default_params(&ctx));

    let allocator = Bump::new();

    let closure = find_closure(repo, requirements.into_iter());

    let mut assert_id = 0;
    let mut assertion_map = HashMap::new();
    let expr_cont = |expr: Bool, sym_expr| {
        let assert_var = Bool::new_const(&ctx, assert_id);
        solver.assert_and_track(&expr.simplify(), &assert_var);
        assertion_map.insert(assert_var, sym_expr);
        assert_id += 1;
    };
    add_all_constraints(
        &allocator,
        &ctx,
        repo,
        closure.iter(),
        requirements,
        expr_cont,
    );

    match solver.check() {
        z3::SatResult::Unsat => {
            let core_vars = solver.get_unsat_core();
            let mut core_assertions = Vec::new();
            for var in core_vars {
                let assertion = assertion_map.get(&var).unwrap_or_else(|| {
                    panic!(
                        "Impossible: unable to find the assertion tracked by the boolean variable {var} in the assertion map"
                    )
                });
                core_assertions.push(assertion);
            }
            let core = process_unsat_core(repo, core_assertions);
            Ok(ResolutionResult::UnsatWithCore { core })
        }
        z3::SatResult::Unknown => Err(ResolutionError::ResolutionFailure {
            reason: solver
                .get_reason_unknown()
                .expect("Impossible: failed to obtain a reason"),
        }),
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Impossible: satisfiable but failed to generate a model");

            let plan = plan_from_model(&ctx, model, closure.iter());

            Ok(ResolutionResult::Sat {
                plans: Vec1::new(plan),
            })
        }
    }
}

pub fn optimize_with(
    repo: &Repository,
    requirements: &RequirementSet,
    gen_metric: impl FnOnce(&Context, Vec<(u32, u64)>, SetU32) -> Vec<Int>,
) -> Res {
    let cfg = default_config();
    let ctx = Context::new(&cfg);
    let solver = Optimize::new(&ctx);

    let allocator = Bump::new();

    let closure = find_closure(repo, requirements.into_iter());

    let package_pairs = closure
        .iter()
        .map(|pid| (pid, repo.newest_ver_of_unchecked(pid)))
        .collect_vec();

    let metrics = gen_metric(&ctx, package_pairs, closure.clone());

    let mut assert_id = 0;
    let mut assertion_map = HashMap::new();
    let expr_cont = |expr: Bool, sym_expr| {
        let assert_var = Bool::new_const(&ctx, assert_id);
        solver.assert_and_track(&expr.simplify(), &assert_var);
        assertion_map.insert(assert_var, sym_expr);
        assert_id += 1;
    };
    add_all_constraints(
        &allocator,
        &ctx,
        repo,
        closure.iter(),
        requirements,
        expr_cont,
    );

    for metric in metrics {
        solver.minimize(&metric);
    }

    match solver.check(&[]) {
        z3::SatResult::Unsat => {
            let core_vars = solver.get_unsat_core();
            let mut core_assertions = Vec::new();
            for var in core_vars {
                let assertion = assertion_map.get(&var).unwrap_or_else(|| {
                    panic!(
                        "Impossible: unable to find the assertion tracked by the boolean variable {var} in the assertion map"
                    )
                });
                core_assertions.push(assertion);
            }
            let core = process_unsat_core(repo, core_assertions);
            Ok(ResolutionResult::UnsatWithCore { core })
        }
        z3::SatResult::Unknown => Err(ResolutionError::ResolutionFailure {
            reason: solver
                .get_reason_unknown()
                .expect("Impossible: failed to obtain a reason"),
        }),
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Impossible: satisfiable but failed to generate a model");

            let plan = plan_from_model(&ctx, model, closure.iter());

            Ok(ResolutionResult::Sat {
                plans: Vec1::new(plan),
            })
        }
    }
}

pub fn optimize_newest(repo: &Repository, requirements: &RequirementSet) -> Res {
    optimize_with(repo, requirements, |ctx, package_pairs, closure| {
        let metric = distance_from_newest(ctx, package_pairs.into_iter());
        let metric2 = installed_packages(ctx, closure.iter());
        vec![metric, metric2]
    })
}

pub fn optimize_minimal(repo: &Repository, requirements: &RequirementSet) -> Res {
    optimize_with(repo, requirements, |ctx, package_pairs, closure| {
        let metric = installed_packages(ctx, closure.iter());
        let metric2 = distance_from_newest(ctx, package_pairs.into_iter());
        vec![metric, metric2]
    })
}

fn parallel_optimize_with<T: Ord>(
    repo: &Repository,
    requirements: &RequirementSet,
    ctx: &Context,
    closure: SetU32,
    eval: impl Fn(&Model) -> T,
) -> Res {
    let solver = Solver::new_for_logic(ctx, "QF_LIA").unwrap();

    let allocator = Bump::new();

    let mut assert_id = 0;
    let mut assertion_map = HashMap::new();
    let expr_cont = |expr: Bool, sym_expr| {
        let assert_var = Bool::new_const(ctx, assert_id);
        solver.assert_and_track(&expr.simplify(), &assert_var);
        assertion_map.insert(assert_var, sym_expr);
        assert_id += 1;
    };
    add_all_constraints(
        &allocator,
        ctx,
        repo,
        closure.iter(),
        requirements,
        expr_cont,
    );

    let vars = closure
        .iter()
        .map(|pid| Int::new_const(ctx, pid))
        .collect::<Vec<_>>();

    match solver.check() {
        z3::SatResult::Unsat => {
            let core_vars = solver.get_unsat_core();
            let mut core_assertions = Vec::new();
            for var in core_vars {
                let assertion = assertion_map.get(&var).unwrap_or_else(|| {
                    panic!(
                        "Impossible: unable to find the assertion tracked by the boolean variable {var} in the assertion map"
                    )
                });
                core_assertions.push(assertion);
            }
            let core = process_unsat_core(repo, core_assertions);
            Ok(ResolutionResult::UnsatWithCore { core })
        }
        z3::SatResult::Unknown => Err(ResolutionError::ResolutionFailure {
            reason: solver
                .get_reason_unknown()
                .expect("Impossible: failed to obtain a reason"),
        }),
        z3::SatResult::Sat => {
            let mut models = Vec::new();
            let cont = |model| models.push(model);

            enumerate_models(&solver, vars.clone().into_iter(), cont);

            let plans_v = iter_max_map(
                models.into_iter(),
                |model| eval(model),
                |model| plan_from_model(ctx, model, closure.iter()),
            );

            let plans = Vec1::try_from(plans_v).expect("Impossible: no plans despite satisfiable");
            Ok(ResolutionResult::Sat { plans })
        }
    }
}

#[deprecated(note = "This function does not actually parallelize and is very slow")]
pub fn parallel_optimize_newest(repo: &Repository, requirements: &RequirementSet) -> Res {
    let closure = find_closure(repo, requirements.into_iter());
    let package_pairs = closure
        .iter()
        .map(|pid| (pid, repo.newest_ver_of_unchecked(pid)));

    let cfg = default_config();
    let ctx = Context::new(&cfg);

    let distance_from_newest_expr = distance_from_newest(&ctx, package_pairs);
    let installed_packages_expr = installed_packages(&ctx, closure.iter());
    parallel_optimize_with(repo, requirements, &ctx, closure, |model| {
        let distance_from_newest = eval_int_expr_in_model(model, &distance_from_newest_expr);
        let installed_packages = eval_int_expr_in_model(model, &installed_packages_expr);
        (distance_from_newest, installed_packages)
    })
}

#[deprecated(note = "This function does not actually parallelize and is very slow")]
pub fn parallel_optimize_minimal(repo: &Repository, requirements: &RequirementSet) -> Res {
    let closure = find_closure(repo, requirements.into_iter());
    let package_pairs = closure
        .iter()
        .map(|pid| (pid, repo.newest_ver_of_unchecked(pid)));

    let cfg = default_config();
    let ctx = Context::new(&cfg);

    let distance_from_newest_expr = distance_from_newest(&ctx, package_pairs);
    let installed_packages_expr = installed_packages(&ctx, closure.iter());
    parallel_optimize_with(repo, requirements, &ctx, closure, |model| {
        let distance_from_newest = eval_int_expr_in_model(model, &distance_from_newest_expr);
        let installed_packages = eval_int_expr_in_model(model, &installed_packages_expr);
        (installed_packages, distance_from_newest)
    })
}

#[cfg(test)]
mod test {
    use crate::{
        solver::{optimize_minimal, optimize_newest},
        types::{Package, PackageVer, Range, Repository, Requirement, RequirementSet},
        z3_helpers::set_global_params,
    };

    use super::simple_solve;

    #[test]
    fn test_simple_solver() {
        let p0 = Package {
            id: 0,
            versions: vec![
                PackageVer {
                    requirements: Default::default(),
                },
                PackageVer {
                    requirements: Default::default(),
                },
                PackageVer {
                    requirements: Default::default(),
                },
                PackageVer {
                    requirements: Default::default(),
                },
            ],
        };
        let p1 = Package {
            id: 1,
            versions: vec![PackageVer {
                requirements: RequirementSet::from_deps(vec![Requirement::new(
                    0,
                    vec1![Range::interval_unchecked(1, 3)],
                )]),
            }],
        };
        let p2 = Package {
            id: 2,
            versions: vec![
                PackageVer {
                    requirements: RequirementSet::from_deps(vec![Requirement::new(
                        0,
                        vec1![Range::interval_unchecked(3, 4)],
                    )]),
                },
                PackageVer {
                    requirements: RequirementSet::from_deps(vec![Requirement::new(
                        0,
                        vec1![Range::interval_unchecked(3, 4)],
                    )]),
                },
            ],
        };
        let mut req_set = RequirementSet::from_deps(vec![Requirement::new(2, vec1![Range::all()])]);
        req_set.add_deps(vec![Requirement::new(
            1,
            vec1![Range::interval_unchecked(1, 1)],
        )]);
        let repo = Repository {
            packages: vec![p0, p1, p2],
        };
        set_global_params();
        let mut r = simple_solve(&repo, &req_set).unwrap();
        println!("{r:?}");
        r = optimize_newest(&repo, &req_set).unwrap();
        println!("{r:?}");
        r = optimize_minimal(&repo, &req_set).unwrap();
        println!("{r:?}");
    }
}
