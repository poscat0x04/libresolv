use crate::types::expr::*;
use crate::types::*;
use crate::utils::merge_and_sort_ranges;
use crate::z3_helpers::zero;
use bumpalo::Bump;
use snafu::{Backtrace, GenerateImplicitData};
use tinyset::SetU32;
use z3::ast::{Ast, Bool, Int};
use z3::Context;

pub fn find_closure<'a, T>(repo: &'a Repository, iter: T) -> Result<SetU32, ResolutionError>
where
    T: Iterator<Item = &'a Requirement>,
{
    let mut s = SetU32::new();
    find_closure_helper(repo, iter, &mut s)?;
    Ok(s)
}

fn find_closure_helper<'a, 'b, T>(
    repo: &'a Repository,
    iter: T,
    acc: &'b mut SetU32,
) -> Result<(), ResolutionError>
where
    T: Iterator<Item = &'a Requirement>,
{
    for req in iter {
        let not_present = acc.insert(req.package);
        if not_present {
            let package = repo.packages.get(req.package as usize).ok_or_else(|| {
                ResolutionError::IllegalIndex {
                    index: req.package,
                    backtrace: Backtrace::generate(),
                }
            })?;
            for ver in &package.versions {
                find_closure_helper(&repo, (&ver.requirements).into_iter(), acc)?;
            }
        }
    }
    Ok(())
}

pub trait AsConstraints {
    fn add_constraints<'a, 'b>(
        &self,
        b: &'b Bump,
        ctx: &'a Context,
        expr_cont: impl FnMut(Bool<'a>, Expr<'b>),
    );
}

impl AsConstraints for Requirement {
    fn add_constraints<'a, 'b>(
        &self,
        b: &'b Bump,
        ctx: &'a Context,
        mut expr_cont: impl FnMut(Bool<'a>, Expr<'b>),
    ) {
        let v = Int::new_const(ctx, self.package);
        let mut expr = Bool::from_bool(ctx, false);
        let mut sym_expr = Expr::bot();

        for r in merge_and_sort_ranges(&self.versions) {
            match r {
                Range::Interval { lower, upper } => {
                    expr |= v.ge(&Int::from_u64(ctx, lower)) & v.le(&Int::from_u64(ctx, upper));
                    let range_expr = Expr::and(
                        b,
                        Expr::Atom(AtomicExpr::ver_ge(self.package, lower)),
                        Expr::Atom(AtomicExpr::ver_le(self.package, upper)),
                    );

                    if sym_expr == Expr::Bot {
                        sym_expr = range_expr
                    } else {
                        sym_expr = Expr::or(b, range_expr, sym_expr)
                    }
                }
                Range::Point(v2) => {
                    expr |= v._eq(&Int::from_u64(ctx, v2));
                    let point_expr = Expr::Atom(AtomicExpr::ver_eq(self.package, v2));

                    if sym_expr == Expr::Bot {
                        sym_expr = point_expr
                    } else {
                        sym_expr = Expr::or(b, point_expr, sym_expr)
                    }
                }
                Range::All => {
                    expr = v._eq(&zero(ctx)).not();
                    sym_expr = Expr::not(b, Expr::Atom(AtomicExpr::ver_eq(self.package, 0)));
                    break;
                }
            }
        }

        expr_cont(expr, sym_expr)
    }
}

impl AsConstraints for RequirementSet {
    fn add_constraints<'a, 'b>(
        &self,
        b: &'b Bump,
        ctx: &'a Context,
        mut expr_cont: impl FnMut(Bool<'a>, Expr<'b>),
    ) {
        for dep in &self.dependencies {
            dep.add_constraints(b, ctx, &mut expr_cont)
        }
        let mut reversed_cont =
            |expr: Bool<'a>, sym_expr| expr_cont(expr.not(), Expr::not(b, sym_expr));
        for antidep in &self.conflicts {
            antidep.add_constraints(b, ctx, &mut reversed_cont)
        }
    }
}

impl AsConstraints for Package {
    fn add_constraints<'a, 'b>(
        &self,
        b: &'b Bump,
        ctx: &'a Context,
        mut expr_cont: impl FnMut(Bool<'a>, Expr<'b>),
    ) {
        let package = Int::new_const(ctx, self.id);
        expr_cont(
            package.ge(&zero(ctx)),
            Expr::Atom(AtomicExpr::ver_ge(self.id, 0)),
        );

        let mut ver_counter = 0;
        for ver in &self.versions {
            ver_counter += 1;
            let ver_number = Int::from_u64(ctx, ver_counter);
            let eq_expr = package._eq(&ver_number);
            let mut modified_cont = |expr, sym_expr| {
                expr_cont(
                    eq_expr.implies(&expr),
                    Expr::implies(
                        b,
                        Expr::Atom(AtomicExpr::ver_eq(self.id, ver_counter)),
                        sym_expr,
                    ),
                )
            };
            ver.requirements.add_constraints(b, ctx, &mut modified_cont);
        }

        expr_cont(
            package.le(&Int::from_u64(ctx, ver_counter)),
            Expr::Atom(AtomicExpr::ver_le(self.id, ver_counter)),
        );
    }
}

pub fn add_all_constraints<'a, 'b>(
    b: &'b Bump,
    ctx: &'a Context,
    repo: &Repository,
    pids: impl Iterator<Item = u32>,
    requirements: &RequirementSet,
    mut expr_cont: impl FnMut(Bool<'a>, Expr<'b>),
) {
    for pid in pids {
        let package = repo.get_package_unchecked(pid);
        package.add_constraints(b, ctx, &mut expr_cont);
    }
    requirements.add_constraints(b, ctx, &mut expr_cont);
}
