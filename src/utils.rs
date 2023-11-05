use std::{
    cmp::{max, min},
    iter::once,
};

use itertools::Itertools;

use crate::types::*;

type Interval = (Version, Version);
type ISet = Vec<Interval>;

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

fn merge_insert(iset: ISet, interval: Interval) -> Vec<Interval> {
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

#[cfg(test)]
mod test {
    use crate::utils::{merge_insert, ISet};

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
