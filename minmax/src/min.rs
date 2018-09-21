use num_traits::Bounded;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    iter::FromIterator,
};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Min<T> {
    value: Option<T>,
}

impl<T> Min<T>
where
    T: Copy + Ord,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_initial(initial: T) -> Self {
        Self {
            value: Some(initial),
        }
    }

    pub fn get_min(&self) -> Option<T> {
        self.value
    }

    pub fn get_min_extreme(&self) -> T
    where
        T: Bounded,
    {
        self.get_min().unwrap_or_else(T::max_value)
    }

    pub fn update<V: Into<Self>>(&mut self, value: V) {
        match (self.value, value.into().value) {
            (None, None) => self.value = None,
            (Some(v), None) | (None, Some(v)) => self.value = Some(v),
            (Some(v1), Some(v2)) => self.value = Some(v1.min(v2)),
        }
    }
}

impl<T> Default for Min<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

impl<T> From<T> for Min<T>
where
    T: Copy + Ord,
{
    fn from(value: T) -> Self {
        Self::with_initial(value)
    }
}

impl<T> FromIterator<T> for Min<T>
where
    T: Ord,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let m = iter.into_iter().min();
        if m.is_some() {
            Self { value: m }
        } else {
            Self::default()
        }
    }
}

impl<T> Display for Min<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        if let Some(v) = &self.value {
            write!(f, "{}", v)
        } else {
            write!(f, "<uninitialized>")
        }
    }
}
