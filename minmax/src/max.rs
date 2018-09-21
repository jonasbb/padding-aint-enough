use num_traits::Bounded;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    iter::FromIterator,
};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Max<T> {
    value: Option<T>,
}

impl<T> Max<T>
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

    pub fn get_max(&self) -> Option<T> {
        self.value
    }

    pub fn get_max_extreme(&self) -> T
    where
        T: Bounded,
    {
        self.get_max().unwrap_or_else(T::min_value)
    }

    pub fn update<V: Into<Self>>(&mut self, value: V) {
        match (self.value, value.into().value) {
            (None, None) => self.value = None,
            (Some(v), None) | (None, Some(v)) => self.value = Some(v),
            (Some(v1), Some(v2)) => self.value = Some(v1.max(v2)),
        }
    }
}

impl<T> Default for Max<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

impl<T> From<T> for Max<T>
where
    T: Copy + Ord,
{
    fn from(value: T) -> Self {
        Self::with_initial(value)
    }
}

impl<T> FromIterator<T> for Max<T>
where
    T: Ord,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let m = iter.into_iter().max();
        if m.is_some() {
            Self { value: m }
        } else {
            Self::default()
        }
    }
}

impl<T> Display for Max<T>
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
