use crate::types::*;
use crate::z3_helpers::zero;
use snafu::{Backtrace, GenerateImplicitData};
use tinyset::SetU32;
use z3::ast::{Ast, Bool, Int};
use z3::Context;

pub fn find_closure<'a, T>(repo: &'a Repository, iter: T) -> Result<SetU32, ResolutionError>
where
    T: IntoIterator<Item = &'a Requirement>,
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
    T: IntoIterator<Item = &'a Requirement>,
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
                find_closure_helper(&repo, &ver.requirements, acc)?;
            }
        }
    }
    Ok(())
}

pub trait AsConstraints {
    fn add_constraints<'a, T>(&self, ctx: &'a Context, cont: T)
    where
        T: FnMut(Bool<'a>) -> ();
}

impl AsConstraints for Requirement {
    fn add_constraints<'a, T>(&self, ctx: &'a Context, mut cont: T)
    where
        T: FnMut(Bool<'a>) -> (),
    {
        let v = Int::new_const(ctx, self.package);
        let exprs: Vec<Bool<'a>> = self
            .versions
            .iter()
            .map(|r| match r {
                Range::Point(v2) => v._eq(&Int::from_u64(ctx, *v2)),
                Range::All => Bool::from_bool(ctx, true),
                Range::Interval { lower, upper } => Bool::and(
                    ctx,
                    &[
                        &v.ge(&Int::from_u64(ctx, *lower)),
                        &v.le(&Int::from_u64(ctx, *upper)),
                    ],
                ),
            })
            .collect();
        cont(Bool::or(ctx, &exprs.iter().collect::<Vec<_>>()[..]));
    }
}

impl AsConstraints for RequirementSet {
    fn add_constraints<'a, T>(&self, ctx: &'a Context, mut cont: T)
    where
        T: FnMut(Bool<'a>) -> (),
    {
        for dep in &self.dependencies {
            dep.add_constraints(ctx, &mut cont)
        }
        let mut reversed_cont = |b: Bool<'a>| cont(b.not());
        for antidep in &self.conflicts {
            antidep.add_constraints(ctx, &mut reversed_cont)
        }
    }
}

impl AsConstraints for Package {
    fn add_constraints<'a, T>(&self, ctx: &'a Context, mut cont: T)
    where
        T: FnMut(Bool<'a>) -> (),
    {
        let package = Int::new_const(ctx, self.id);
        cont(package.ge(&zero(ctx)));

        let mut ver_counter = 0;
        for ver in &self.versions {
            ver_counter += 1;
            let ver_number = Int::from_u64(ctx, ver_counter);
            let eq_expr = package._eq(&ver_number);
            let mut modified_cont = |b: Bool<'a>| cont(eq_expr.implies(&b));
            ver.requirements.add_constraints(ctx, &mut modified_cont);
        }

        cont(package.le(&Int::from_u64(ctx, ver_counter)));
    }
}

fn add_all_constraints<'a, T>(
    ctx: &'a Context,
    repo: &Repository,
    requirements: &RequirementSet,
    mut cont: T,
) -> Result<(), ResolutionError>
where
    T: FnMut(Bool<'a>) -> (),
{
    let pids = find_closure(repo, requirements)?;
    for pid in pids {
        let package = repo.get_package_unchecked(pid);
        package.add_constraints(ctx, &mut cont);
    }
    requirements.add_constraints(ctx, &mut cont);
    Ok(())
}
