use crate::{
    constraints::{add_all_constraints, find_closure},
    types::*,
    z3_helpers::default_params,
};
use snafu::{Backtrace, GenerateImplicitData};
use z3::{ast::Int, Config, Context, Model, Optimize, Solver, Tactic};

fn plan_from_model(ctx: &Context, model: Model, pids: impl Iterator<Item = PackageId>) -> Plan {
    let mut plan = Vec::new();
    let mut no_interp = Vec::new();
    let mut interp_not_u64 = Vec::new();

    for package_id in pids {
        let p = Int::new_const(&ctx, package_id);
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

pub fn simple_solve(cfg: &Config, repo: &Repository, requirements: &RequirementSet) -> Res {
    let ctx = Context::new(&cfg);
    let tactic = Tactic::new(&ctx, "simplify").and_then(&Tactic::new(&ctx, "psat"));
    let solver = tactic.solver();
    solver.set_params(&default_params(&ctx));

    let closure = find_closure(repo, (&requirements).into_iter())?;

    let expr_cont = |b| solver.assert(&b);
    add_all_constraints(&ctx, repo, closure.iter(), requirements, expr_cont);

    match solver.check() {
        z3::SatResult::Unsat => Ok(ResolutionResult::Unsat),
        z3::SatResult::Unknown => Err(ResolutionError::TimeOut {
            backtrace: Backtrace::generate(),
        }),
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Impossible: satisfiable but faild to generate a model");

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
                    vec![Range::interval_unchecked(1, 2)],
                )]),
            }],
        };
        let p2 = Package {
            id: 2,
            versions: vec![PackageVer {
                requirements: RequirementSet::from_deps(vec![Requirement::new(
                    0,
                    vec![Range::interval_unchecked(2, 3)],
                )]),
            }],
        };
        let req_set = RequirementSet::from_deps(vec![
            Requirement::new(1, vec![Range::all()]),
            Requirement::new(2, vec![Range::all()]),
        ]);
        let repo = Repository {
            packages: vec![p0, p1, p2],
        };
        set_global_params();
        let cfg = default_config();
        let r = simple_solve(&cfg, &repo, &req_set).unwrap();
        println!("{r:?}");
    }
}
