use std::collections::HashMap;

use crate::{
    constraints::{add_all_constraints, find_closure},
    types::{
        expr::{AtomicExpr, Expr},
        *,
    },
    z3_helpers::default_params,
};
use bumpalo::Bump;
use intmap::IntMap;
use snafu::{Backtrace, GenerateImplicitData};
use z3::{
    ast::{Ast, Bool, Int},
    Config, Context, Model, Solver,
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
                        conflicts.push(Requirement::new(*pid, vec![Range::all()]))
                    } else {
                        dependencies.push(Requirement::new(*pid, vec![Range::point(*version)]))
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
                        req = Some(Requirement::new(*pid2, vec![Range::all()]));
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
    let (pid, ranges) = process_version_range_helper(expr);
    Requirement::new(pid, ranges)
}

fn process_version_range_helper(expr: &Expr<'_>) -> (PackageId, Vec<Range>) {
    let panic = || panic!("Impossible: unknown expression {expr} for version range(s)");
    match expr {
        Expr::Atom(AtomicExpr::VerEq { pid, version }) => (*pid, vec![Range::point(*version)]),
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
            let rs = vec![Range::interval(lb, ub).unwrap_or_else(|| {
                panic!("Impossible: lower bound is bigger than upper bound in expr {expr}")
            })];
            (package_id, rs)
        }
        Expr::Or(lhs, rhs) => {
            let (pid1, mut rs1) = process_version_range_helper(lhs);
            let (pid2, mut rs2) = process_version_range_helper(rhs);
            assert_eq!(pid1, pid2);
            rs1.append(&mut rs2);
            (pid1, rs1)
        }
        Expr::Not(Expr::Atom(AtomicExpr::VerEq { pid, version: 0 })) => (*pid, vec![Range::all()]),
        _ => panic(),
    }
}

pub fn simple_solve(cfg: &Config, repo: &Repository, requirements: &RequirementSet) -> Res {
    let ctx = Context::new(cfg);
    let solver = Solver::new_for_logic(&ctx, "QF_FD").unwrap();
    solver.set_params(&default_params(&ctx));

    let allocator = Bump::new();

    let closure = find_closure(repo, requirements.into_iter())?;

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
        z3::SatResult::Unknown => Err(ResolutionError::TimeOut {
            backtrace: Backtrace::generate(),
        }),
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Impossible: satisfiable but failed to generate a model");

            let plan = plan_from_model(&ctx, model, closure.iter());

            Ok(ResolutionResult::Sat { plan })
        }
    }
}

pub fn optimize_newest(cfg: &Config, repo: &Repository, requirement: &RequirementSet) -> Res {
    todo!()
}

pub fn optimize_minimal(cfg: &Config, repo: &Repository, requirement: &RequirementSet) -> Res {
    todo!()
}

pub fn parallel_optimize_newest(
    cfg: &Config,
    repo: &Repository,
    requirement: &RequirementSet,
) -> Res {
    todo!()
}

pub fn parallel_optimize_minimal(
    cfg: &Config,
    repo: &Repository,
    requirement: &RequirementSet,
) -> Res {
    todo!()
}

#[cfg(test)]
mod test {
    use crate::{
        types::{Package, PackageVer, Range, Repository, Requirement, RequirementSet},
        z3_helpers::{default_config, set_global_params},
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
                    vec![Range::interval_unchecked(1, 3)],
                )]),
            }],
        };
        let p2 = Package {
            id: 2,
            versions: vec![
                PackageVer {
                    requirements: RequirementSet::from_deps(vec![Requirement::new(
                        0,
                        vec![Range::interval_unchecked(4, 4)],
                    )]),
                },
                PackageVer {
                    requirements: RequirementSet::from_deps(vec![Requirement::new(
                        0,
                        vec![Range::interval_unchecked(4, 4)],
                    )]),
                },
            ],
        };
        let mut req_set =
            RequirementSet::from_deps(vec![Requirement::new(2, vec![Range::point(1)])]);
        req_set.add_deps(vec![Requirement::new(
            1,
            vec![Range::interval_unchecked(1, 1)],
        )]);
        let repo = Repository {
            packages: vec![p0, p1, p2],
        };
        set_global_params();
        let cfg = default_config();
        let r = simple_solve(&cfg, &repo, &req_set).unwrap();
        println!("{r:?}");
    }
}
