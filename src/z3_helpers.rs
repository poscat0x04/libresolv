use crate::types::*;
use z3::ast::{Ast, Bool, Int};
use z3::SatResult::Sat;
use z3::{set_global_param, Config, Context, Model, Params, Solver};

pub fn set_global_params() {
    set_global_param("unsat_core", "true");
    set_global_param("parallel.enable", "true");
    set_global_param("sat.core.minimize", "true");
    set_global_param("sat.threads", "12");
    set_global_param("smt.core.minimize", "true");
    set_global_param("smt.threads", "12");
}

pub fn default_params(ctx: &Context) -> Params<'_> {
    let mut p = Params::new(ctx);
    p.set_bool("unsat_core", true);
    p.set_bool("core.minimize", true);
    p
}

pub fn default_config() -> Config {
    let mut cfg = Config::new();
    cfg.set_bool_param_value("unsat_core", true);
    cfg
}

pub fn zero(ctx: &Context) -> Int {
    Int::from_u64(ctx, 0)
}

// sgn function
pub fn sgn<'a>(ctx: &'a Context, a: Int<'a>) -> Int<'a> {
    a.gt(&zero(ctx)).ite(
        &Int::from_u64(ctx, 1),
        &a.lt(&zero(ctx)).ite(&Int::from_i64(ctx, -1), &zero(ctx)),
    )
}

// the expression representing the taxicab distance of all installed from the newest versions,
// useful as an optimization metric
pub fn distance_from_newest(
    ctx: &Context,
    iter: impl Iterator<Item = (PackageId, Version)>,
) -> Int {
    let mut expr = zero(ctx);
    for (pid, max_ver) in iter {
        let pkg_ver = Int::new_const(ctx, pid);
        expr += pkg_ver
            ._eq(&zero(ctx))
            .ite(&zero(ctx), &(Int::from_u64(ctx, max_ver) - pkg_ver));
    }
    expr.simplify()
}

// the expression representing the number of packages installed, useful as an optimization metric
pub fn installed_packages(ctx: &Context, pids: impl Iterator<Item = PackageId>) -> Int {
    let mut expr = zero(ctx);
    for pid in pids {
        expr += sgn(ctx, Int::new_const(ctx, pid));
    }
    expr.simplify()
}

pub fn eval_int_expr_in_model(model: &Model, expr: &Int) -> u64 {
    let eval_result = model
        .eval(expr, false)
        .unwrap_or_else(|| panic!("Impossible: failed to evaluate expression {expr} in model"));
    eval_result
        .as_u64()
        .unwrap_or_else(|| panic!("Impossible: failed to convert eval result {eval_result} to u64"))
}

// enumerate all models.
pub fn enumerate_models<'a, T: Ast<'a>>(
    solver: &'a Solver,
    vars: impl Iterator<Item = T> + Clone,
    mut cont: impl FnMut(Model<'a>),
) {
    fn block_var<'a, T: Ast<'a>>(solver: &'a Solver, model: &Model<'a>, var: &T) {
        let assertion = var
            ._eq(&model.eval(var, false).unwrap_or_else(|| {
                panic!("unable to find an interpretation for variable {var:?} in model")
            }))
            .not();
        solver.assert(&assertion);
    }

    fn fix_var<'a, T: Ast<'a>>(solver: &'a Solver, model: &Model<'a>, var: &T) {
        let assertion = var._eq(&model.eval(var, false).unwrap_or_else(|| {
            panic!("unable to find an interpretation for variable {var:?} in model",)
        }));
        solver.assert(&assertion);
    }

    fn get_model<'a>(solver: &'a Solver) -> Model<'a> {
        solver
            .get_model()
            .expect("Impossible: failed to get a model despite being satisifable")
    }

    // model enumeration: we use the method described in https://stackoverflow.com/questions/11867611/z3py-checking-all-solutions-for-equation
    // to reuse each learnt lemma as much as possible
    //
    // we first first try to find a model, if this fails than the theory is unsatisfiable and the
    // enumeration is complete.
    // then for all the variables in our theory (which is { "k!i" | i in "closure of package" }),
    // we pick a variable to be enumerated first, then we fix all other variables to their interpretations
    // in this model, and enumerate all possible interpretations of this specific variable (by keeping
    // adding assertions blocking enumerated values), after we've hit an "unsat", we backtrack, pop out
    // all the assertions created during enumeration, and the assertion fixing the second variable, instead
    // we add an assertion blocking the second variable, tries to find a new model, then we fix the second
    // variable, repeat the enumeration step for the first variable, and so on to enumerate the scecond variable,
    // after that we backtrack to the third variable, and fourth... until all the variable has been enumerated.
    fn go<'a, T: Ast<'a>>(
        solver: &'a Solver,
        cont: &mut impl FnMut(Model<'a>),
        mut vars: impl Iterator<Item = T> + Clone,
    ) {
        if let Some(var) = vars.next() {
            solver.push();
            while solver.check() == Sat {
                let model = get_model(solver);
                solver.push();
                fix_var(solver, &model, &var);
                go(solver, cont, vars.clone());
                solver.pop(1);
                block_var(solver, &model, &var);
            }
            solver.pop(1);
        } else if solver.check() == Sat {
            cont(get_model(solver));
        }
    }
    go(solver, &mut cont, vars);
}

pub fn installation_status(
    ctx: &Context,
    model: &Model,
    closure: impl Iterator<Item = PackageId>,
) -> (Vec<PackageId>, Vec<PackageId>) {
    let mut not_installed = Vec::new();
    let mut installed = Vec::new();
    for pid in closure {
        let p = Int::new_const(ctx, pid);
        if let Some(interp) = model.get_const_interp(&p) {
            if let Some(v) = interp.as_u64() {
                if v != 0 {
                    installed.push(pid);
                    continue;
                }
            }
        }
        not_installed.push(pid);
    }
    (installed, not_installed)
}

pub fn fix_installed_pkgs(ctx: &Context, solver: &Solver, not_installed: &Vec<PackageId>) {
    for pid in not_installed {
        solver.assert(&Int::new_const(ctx, *pid)._eq(&zero(ctx)));
    }
}

pub fn block_le_solutions(
    ctx: &Context,
    solver: &Solver,
    model: &Model,
    installed: &Vec<PackageId>,
) {
    let mut e = Bool::from_bool(ctx, true);
    for pid in installed {
        let p = Int::new_const(ctx, *pid);
        let v = eval_int_expr_in_model(model, &p);
        let v_expr = Int::from_u64(ctx, v);
        e &= p._eq(&v_expr);
        solver.assert(&p.ge(&v_expr));
    }
    e = e.not().simplify();
    solver.assert(&e);
}

#[cfg(test)]
mod test {
    use crate::z3_helpers::{default_config, set_global_params};
    use z3::ast::{Ast, Bool, Int};
    use z3::{Context, Goal, Solver, Tactic};

    #[test]
    fn test_build_context() {
        set_global_params();
        let cfg = default_config();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);
        let v = Int::new_const(&ctx, 1);
        let expr = Bool::from_bool(&ctx, true)
            & (v.ge(&Int::from_u64(&ctx, 1)) & v.ge(&Int::from_u64(&ctx, 2)));
        println!("{:?}", expr);

        let tactic = Tactic::new(&ctx, "propagate-ineqs");
        let goal = Goal::new(&ctx, false, false, false);
        goal.assert(&expr);
        let r = tactic.apply(&goal, None).unwrap();
        for res in r.list_subgoals() {
            println!("{}", res)
        }

        println!("{:?}", expr.decl().arity());
        solver.assert(&expr.not());
        println!("{:?}", solver.check());
        let model = solver.get_model().unwrap();
        let assigned_value = model.get_const_interp(&v).unwrap();
        println!("{:?}", assigned_value.as_u64());
    }

    #[test]
    fn test_iter_clone() {
        let v = [1, 2, 3, 4, 5, 6, 7];
        let mut iter = v.iter().map(|x| x + 1);
        iter.next();
        iter.next();
        for i in iter.clone() {
            println!("{i}")
        }
        for i in iter.clone() {
            println!("{i}")
        }
        for i in iter {
            println!("o: {i}")
        }
    }
}
