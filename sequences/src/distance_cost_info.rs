//! [DistanceCostInfo](crate::distance_cost_info::DistanceCostInfo) trait and implementations of it
//!
//! The trait is used to track how the costs for a single distance are split accross the individual
//! components.
use crate::SequenceElement;
use std::{collections::BTreeMap, sync::Arc};

pub trait DistanceCostInfo: Clone + Default {
    /// Indicates that the insert operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn insert(&self, cost: usize, elem1: SequenceElement) -> Self;
    /// Indicates that the delete operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn delete(&self, cost: usize, elem1: SequenceElement) -> Self;
    /// Indicates that the substitute operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn substitute(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self;
    /// Indicates that the swap operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn swap(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self;
    /// Indicates that the distance computation was aborted early.
    ///
    /// This occurs, if the current distance is already larger than any distance in the kNN set.
    #[must_use]
    fn abort(&self) -> Self;
}

impl DistanceCostInfo for () {
    fn insert(&self, _cost: usize, _elem1: SequenceElement) -> Self {}
    fn delete(&self, _cost: usize, _elem1: SequenceElement) -> Self {}
    fn substitute(&self, _cost: usize, _elem1: SequenceElement, _elem2: SequenceElement) -> Self {}
    fn swap(&self, _cost: usize, _elem1: SequenceElement, _elem2: SequenceElement) -> Self {}
    fn abort(&self) -> Self {}
}

#[derive(Debug, Clone, Default)]
pub struct CostTracker {
    pub insert_gap: usize,
    pub insert_size: usize,
    pub delete_gap: usize,
    pub delete_size: usize,
    pub substitute_gap_gap: usize,
    pub substitute_gap_size: usize,
    pub substitute_size_gap: usize,
    pub substitute_size_size: usize,
    pub swap_gap_gap: usize,
    pub swap_gap_size: usize,
    pub swap_size_gap: usize,
    pub swap_size_size: usize,
    pub is_abort: bool,
    pub from_gap_to_gap: Arc<BTreeMap<(u16, u16), usize>>,
    current_cost: usize,
}

impl CostTracker {
    pub fn as_btreemap(&self) -> BTreeMap<String, usize> {
        let mut res = BTreeMap::default();

        // Convert all the gap-to-gap counts
        for ((from, to), &count) in &*self.from_gap_to_gap {
            res.insert(format!("gap({})_to_gap({})", from, to), count);
        }

        res.insert("insert_gap".into(), self.insert_gap);
        res.insert("insert_size".into(), self.insert_size);
        res.insert("delete_gap".into(), self.delete_gap);
        res.insert("delete_size".into(), self.delete_size);
        res.insert("substitute_gap_gap".into(), self.substitute_gap_gap);
        res.insert("substitute_gap_size".into(), self.substitute_gap_size);
        res.insert("substitute_size_gap".into(), self.substitute_size_gap);
        res.insert("substitute_size_size".into(), self.substitute_size_size);
        res.insert("swap_gap_gap".into(), self.swap_gap_gap);
        res.insert("swap_gap_size".into(), self.swap_gap_size);
        res.insert("swap_size_gap".into(), self.swap_size_gap);
        res.insert("swap_size_size".into(), self.swap_size_size);
        res.insert("is_abort".into(), self.is_abort as usize);
        res
    }

    fn update<F>(&self, cost: usize, f: F) -> Self
    where
        F: Fn(&mut Self, usize),
    {
        let mut res = self.clone();
        let diff = cost - self.current_cost;
        res.current_cost = cost;
        f(&mut res, diff);
        res
    }
}

impl DistanceCostInfo for CostTracker {
    fn insert(&self, cost: usize, elem1: SequenceElement) -> Self {
        self.update(cost, |x, diff| match elem1 {
            SequenceElement::Gap(_) => x.insert_gap += diff,
            SequenceElement::Size(_) => x.insert_size += diff,
        })
    }
    fn delete(&self, cost: usize, elem1: SequenceElement) -> Self {
        self.update(cost, |x, diff| match elem1 {
            SequenceElement::Gap(_) => x.delete_gap += diff,
            SequenceElement::Size(_) => x.delete_size += diff,
        })
    }
    fn substitute(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self {
        let mut this = self.clone();
        if self.current_cost != cost {
            if let (SequenceElement::Gap(g1), SequenceElement::Gap(g2)) = (elem1, elem2) {
                let bmap = Arc::make_mut(&mut this.from_gap_to_gap);
                let min = g1.min(g2);
                let max = g1.max(g2);
                *bmap.entry((min, max)).or_insert(0) += 1;
            }
        }
        this.update(cost, |x, diff| match (elem1, elem2) {
            (SequenceElement::Gap(_), SequenceElement::Gap(_)) => x.substitute_gap_gap += diff,
            (SequenceElement::Gap(_), SequenceElement::Size(_)) => x.substitute_gap_size += diff,
            (SequenceElement::Size(_), SequenceElement::Gap(_)) => x.substitute_size_gap += diff,
            (SequenceElement::Size(_), SequenceElement::Size(_)) => x.substitute_size_size += diff,
        })
    }
    fn swap(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self {
        self.update(cost, |x, diff| match (elem1, elem2) {
            (SequenceElement::Gap(_), SequenceElement::Gap(_)) => x.swap_gap_gap += diff,
            (SequenceElement::Gap(_), SequenceElement::Size(_)) => x.swap_gap_size += diff,
            (SequenceElement::Size(_), SequenceElement::Gap(_)) => x.swap_size_gap += diff,
            (SequenceElement::Size(_), SequenceElement::Size(_)) => x.swap_size_size += diff,
        })
    }
    fn abort(&self) -> Self {
        let mut res = self.clone();
        res.is_abort = true;
        res
    }
}
