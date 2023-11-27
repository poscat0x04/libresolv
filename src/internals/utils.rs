pub mod z3;

use std::cmp::Ordering;

pub use colors::{blue_text, green_text, red_text};
pub use interval_merging::merge_and_sort_ranges;
pub use z3::*;

// Utilities for merging intervals
mod interval_merging {
    use crate::internals::types::*;
    use itertools::Itertools;
    use std::{
        cmp::{max, min},
        iter::once,
    };

    pub type Interval = (Version, Version);
    pub type ISet = Vec<Interval>;

    fn less_no_overlap(a: Interval, b: Interval) -> bool {
        (a.1 + 1) < b.0
    }

    fn greater_no_overlap(a: Interval, b: Interval) -> bool {
        less_no_overlap(b, a)
    }

    fn overlaps(a: Interval, b: Interval) -> bool {
        !(less_no_overlap(a, b) || greater_no_overlap(a, b))
    }

    fn merge(a: Interval, b: Interval) -> Interval {
        (min(a.0, b.0), max(a.1, b.1))
    }

    pub fn merge_insert(iset: ISet, interval: Interval) -> Vec<Interval> {
        let mut result = Vec::new();
        let mut merged = interval;

        let mut iset_iter = iset.iter();
        let init_iter = iset_iter
            .by_ref()
            .peeking_take_while(|i| less_no_overlap(**i, interval));

        for i in init_iter {
            result.push(*i);
        }

        let overlapping_iter = iset_iter
            .by_ref()
            .peeking_take_while(|i| overlaps(**i, interval));

        for i in overlapping_iter {
            merged = merge(merged, *i);
        }
        result.push(merged);

        for i in iset_iter {
            result.push(*i);
        }

        result
    }

    // TODO: add tests
    pub fn merge_and_sort_ranges(ranges: &Vec<Range>) -> Box<dyn Iterator<Item = Range>> {
        let mut iset: Vec<Interval> = Vec::new();
        for range in ranges {
            match range {
                Range::Interval { lower, upper } => iset = merge_insert(iset, (*lower, *upper)),
                Range::Point(p) => iset = merge_insert(iset, (*p, *p)),
                Range::All => return Box::new(once(Range::All)),
            }
        }

        Box::new(iset.into_iter().map(|(l, u)| {
            if l == u {
                Range::Point(l)
            } else {
                Range::Interval { lower: l, upper: u }
            }
        }))
    }
}

// Select the maximum value(s) from an iterator using an evaluation function
// while simultaneously mapping the values using another function
pub fn iter_max_map<T, V: Ord, W>(
    iter: impl Iterator<Item = T>,
    eval: impl Fn(&T) -> V,
    f: impl Fn(T) -> W,
) -> Vec<W> {
    let mut cur: Option<V> = None;
    let mut v = Vec::new();
    for i in iter {
        if let Some(ref c) = cur {
            let e = eval(&i);
            match c.cmp(&e) {
                Ordering::Equal => v.push(f(i)),
                Ordering::Less => {
                    cur = Some(e);
                    v.clear();
                    v.push(f(i))
                }
                _ => {}
            }
        } else {
            cur = Some(eval(&i));
            v.push(f(i));
        }
    }
    v
}

// Colors for terminal displaying
mod colors {
    use termcolor::{Color, ColorSpec};
    pub fn green_text() -> ColorSpec {
        let mut c = ColorSpec::new();
        c.set_fg(Some(Color::Ansi256(76)));
        c
    }

    pub fn red_text() -> ColorSpec {
        let mut c = ColorSpec::new();
        c.set_fg(Some(Color::Ansi256(196)));
        c
    }

    pub fn blue_text() -> ColorSpec {
        let mut c = ColorSpec::new();
        c.set_fg(Some(Color::Ansi256(39)));
        c
    }
}

#[cfg(test)]
mod test {
    use crate::internals::utils::interval_merging::{merge_insert, ISet};

    #[test]
    fn test_merge_insert() {
        let mut i1: ISet = vec![(0, 1), (5, 6)];

        i1 = merge_insert(i1, (3, 3));
        assert_eq!(i1, vec![(0, 1), (3, 3), (5, 6)]);

        let mut i2: ISet = vec![(0, 1), (3, 4), (7, 8)];
        i2 = merge_insert(i2, (2, 6));
        assert_eq!(i2, vec![(0, 8)]);
    }
}
