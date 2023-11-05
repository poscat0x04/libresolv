use crate::types::*;
use z3::ast::{Ast, Int};
use z3::{set_global_param, Config, Context};

pub fn set_params() {
    set_global_param("unsat_core", "true");
    set_global_param("parallel.enable", "true");
    set_global_param("smt.core.minimize", "true");
    set_global_param("smt.threads", "12");
}

pub fn default_config() -> Config {
    let mut cfg = Config::new();
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

// the taxicab distance of all installed from the newest versions, useful as an optimization metric
pub fn distance_from_newest(
    ctx: &Context,
    iter: impl IntoIterator<Item = (PackageId, Version)>,
) -> Int {
    let mut expr = zero(ctx);
    for (pid, max_ver) in iter {
        let pkg_ver = Int::new_const(ctx, pid);
        expr += pkg_ver
            ._eq(&zero(ctx))
            .ite(&zero(ctx), &(Int::from_u64(ctx, max_ver) - pkg_ver));
    }
    expr
}

// the number of packages installed, useful as an optimization metric
pub fn installed_packages(ctx: &Context, pids: impl IntoIterator<Item = PackageId>) -> Int {
    let mut expr = zero(ctx);
    for pid in pids {
        expr += sgn(ctx, Int::new_const(ctx, pid));
    }
    expr
}

#[cfg(test)]
mod test {
    use crate::z3_helpers::{default_config, set_params};
    use z3::ast::{Ast, Int};
    use z3::{Context, Solver};

    #[test]
    fn test_build_context() {
        set_params();
        let cfg = default_config();
        let ctx = Context::new(&cfg);
        let mut solver = Solver::new(&ctx);
        let v = Int::new_const(&ctx, 1);
        let v2 = Int::new_const(&ctx, 1);
        println!("{:?}", v._eq(&Int::from_u64(&ctx, 0)));
        solver.assert(&v._eq(&Int::from_u64(&ctx, 0)).not());
        println!("{:?}", solver.check());
        let model = solver.get_model().unwrap();
        let assigned_value = model.get_const_interp(&v).unwrap();
        println!("{:?}", assigned_value.as_u64());
    }
}
