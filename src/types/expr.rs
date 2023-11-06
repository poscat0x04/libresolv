// Symbolic formulas, we generate them at the same time as we generate the
// constraints for z3. This way we can avoid the painful process of parsing
// z3 ASTs
use std::cmp::Ordering;
use std::fmt::{self, Formatter};

use bumpalo::Bump;

use crate::types::*;

pub trait DisplayPrec {
    type Prec: PartialOrd;
    fn fmt_prec(&self, prec: Self::Prec, fmt: &mut Formatter<'_>) -> fmt::Result;
}

#[repr(transparent)]
pub struct ViaDisplayPrec<'a, T>(&'a T);

impl<T, V> Display for ViaDisplayPrec<'_, T>
where
    V: Default,
    T: DisplayPrec<Prec = V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt_prec(Default::default(), f)
    }
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum AtomicExpr {
    VerEq { pid: PackageId, version: Version },
    VerLE { pid: PackageId, version: Version },
    VerGE { pid: PackageId, version: Version },
}

impl Display for AtomicExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerEq { pid, version } => write!(f, "Ver({pid}) = {version}"),
            Self::VerLE { pid, version } => write!(f, "Ver({pid}) ≤ {version}"),
            Self::VerGE { pid, version } => write!(f, "Ver({pid}) ≥ {version}"),
        }
    }
}

impl AtomicExpr {
    pub fn ver_eq(pid: PackageId, version: Version) -> AtomicExpr {
        AtomicExpr::VerEq { pid, version }
    }

    pub fn ver_le(pid: PackageId, version: Version) -> AtomicExpr {
        AtomicExpr::VerLE { pid, version }
    }

    pub fn ver_ge(pid: PackageId, version: Version) -> AtomicExpr {
        AtomicExpr::VerGE { pid, version }
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum Expr<'a> {
    Atom(AtomicExpr),
    Not(&'a Expr<'a>),
    And(&'a Expr<'a>, &'a Expr<'a>),
    Or(&'a Expr<'a>, &'a Expr<'a>),
    Implies(&'a Expr<'a>, &'a Expr<'a>),
    Bot,
    Top,
}

impl std::fmt::Debug for Expr<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ViaDisplayPrec(self).fmt(f)
    }
}

impl std::fmt::Display for Expr<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ViaDisplayPrec(self).fmt(f)
    }
}

impl Expr<'_> {
    pub fn atom<'a>(expr: AtomicExpr) -> Expr<'a> {
        Expr::Atom(expr)
    }

    pub fn not<'a>(b: &'a Bump, expr: Expr<'a>) -> Expr<'a> {
        match expr {
            Expr::Not(inner) => inner.clone(),
            _ => Expr::Not(b.alloc(expr)),
        }
    }

    pub fn and<'a>(b: &'a Bump, expr1: Expr<'a>, expr2: Expr<'a>) -> Expr<'a> {
        Expr::And(b.alloc(expr1), b.alloc(expr2))
    }

    pub fn or<'a>(b: &'a Bump, expr1: Expr<'a>, expr2: Expr<'a>) -> Expr<'a> {
        Expr::Or(b.alloc(expr1), b.alloc(expr2))
    }

    pub fn implies<'a>(b: &'a Bump, expr1: Expr<'a>, expr2: Expr<'a>) -> Expr<'a> {
        Expr::Implies(b.alloc(expr1), b.alloc(expr2))
    }

    pub fn bot<'a>() -> Expr<'a> {
        Expr::Bot
    }

    pub fn top<'a>() -> Expr<'a> {
        Expr::Top
    }
}

// "chaining" two posets together
#[derive(Eq, PartialEq, Debug)]
pub enum Chain<T, V> {
    Top(T),
    Bot(V),
}

impl<T, B> PartialOrd for Chain<T, B>
where
    T: PartialOrd,
    B: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::Top(_), Self::Bot(_)) => Some(Ordering::Greater),
            (Self::Bot(_), Self::Top(_)) => Some(Ordering::Less),
            (Self::Top(t1), Self::Top(t2)) => t1.partial_cmp(t2),
            (Self::Bot(b1), Self::Bot(b2)) => b1.partial_cmp(b2),
        }
    }
}

// "antichaining" two posets together
#[derive(Eq, PartialEq, Debug)]
pub enum AntiChain<L, R> {
    Left(L),
    Right(R),
}

impl<T, V> PartialOrd for AntiChain<T, V>
where
    T: PartialOrd,
    V: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::Left(_), Self::Right(_)) => None,
            (Self::Right(_), Self::Left(_)) => None,
            (Self::Left(t1), Self::Left(t2)) => t1.partial_cmp(t2),
            (Self::Right(b1), Self::Right(b2)) => b1.partial_cmp(b2),
        }
    }
}

pub type ExprPrec = Chain<u8, Chain<AntiChain<(), ()>, u8>>;

const OUTER_PREC: ExprPrec = Chain::Bot(Chain::Bot(0));
const NOT_PREC: ExprPrec = Chain::Top(0);
const AND_PREC: ExprPrec = Chain::Bot(Chain::Top(AntiChain::Left(())));
const OR_PREC: ExprPrec = Chain::Bot(Chain::Top(AntiChain::Right(())));
const IMPL_PREC: ExprPrec = Chain::Bot(Chain::Bot(1));
const IMPL_PREC_L: ExprPrec = Chain::Bot(Chain::Bot(2));

impl Default for ExprPrec {
    fn default() -> Self {
        OUTER_PREC
    }
}

impl DisplayPrec for Expr<'_> {
    type Prec = ExprPrec;
    fn fmt_prec(&self, prec: ExprPrec, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Atom(a) => a.fmt(f),
            Self::Not(e) => {
                write!(f, "¬")?;
                e.fmt_prec(NOT_PREC, f)
            }
            Self::And(l, r) => {
                if !(prec <= AND_PREC) {
                    write!(f, "(")?;
                }
                l.fmt_prec(AND_PREC, f)?;
                write!(f, " ∧ ")?;
                r.fmt_prec(AND_PREC, f)?;
                if !(prec <= AND_PREC) {
                    write!(f, ")")?;
                }
                Ok(())
            }
            Self::Or(l, r) => {
                if !(prec <= OR_PREC) {
                    write!(f, "(")?;
                }
                l.fmt_prec(OR_PREC, f)?;
                write!(f, " ∨ ")?;
                r.fmt_prec(OR_PREC, f)?;
                if !(prec <= OR_PREC) {
                    write!(f, ")")?;
                }
                Ok(())
            }
            Self::Implies(l, r) => {
                if !(prec <= IMPL_PREC) {
                    write!(f, "(")?;
                }
                l.fmt_prec(IMPL_PREC_L, f)?;
                write!(f, " → ")?;
                r.fmt_prec(IMPL_PREC, f)?;
                if !(prec <= IMPL_PREC) {
                    write!(f, ")")?;
                }
                Ok(())
            }
            Self::Bot => write!(f, "⊤"),
            Self::Top => write!(f, "⊥"),
        }
    }
}

#[cfg(test)]
mod test {
    use bumpalo::Bump;

    use crate::types::expr::ViaDisplayPrec;

    use super::{AtomicExpr, Expr};

    #[test]
    fn test_pretty_printing() {
        let b = Bump::new();
        let a1 = b.alloc(Expr::Atom(AtomicExpr::VerEq { pid: 1, version: 1 }));
        let a2 = b.alloc(Expr::Atom(AtomicExpr::VerEq { pid: 2, version: 1 }));
        let expr1 = Expr::Or(b.alloc(Expr::And(a1, a2)), a1);
        println!("{}", ViaDisplayPrec(&expr1));
        let expr2 = Expr::Or(a1, b.alloc(Expr::And(a2, a1)));
        println!("{}", ViaDisplayPrec(&expr2));
        let expr3 = Expr::Or(b.alloc(Expr::Implies(a1, a2)), a1);
        println!("{}", ViaDisplayPrec(&expr3));
        let expr4 = Expr::And(b.alloc(Expr::And(a1, a2)), a1);
        println!("{}", ViaDisplayPrec(&expr4));
        let expr5 = Expr::Implies(b.alloc(Expr::Implies(a1, a2)), a1);
        println!("{}", ViaDisplayPrec(&expr5));
        let expr6 = Expr::Implies(a1, b.alloc(Expr::Implies(a1, a2)));
        println!("{}", ViaDisplayPrec(&expr6));
        let expr7 = Expr::Implies(
            b.alloc(Expr::Implies(b.alloc(Expr::Implies(a1, a2)), a1)),
            a2,
        );
        println!("{}", ViaDisplayPrec(&expr7));
    }
}
