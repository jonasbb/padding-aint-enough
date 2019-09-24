use misc_utils::{Max, Min};
use num_traits::float::Float;
use ordered_float::NotNan;
use serde::de::{Deserializer, Error};
use serde_with::rust::display_fromstr;
use std::{fmt::Display, str::FromStr};

/// Deserialize T using [FromStr]
pub fn deserialize_min_notnan<'de, D, T>(deserializer: D) -> Result<Min<NotNan<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + Float,
    T::Err: Display,
{
    let value: T = display_fromstr::deserialize(deserializer)?;
    let value = NotNan::new(value).map_err(Error::custom)?;
    Ok(Min::with_initial(value))
}

/// Deserialize T using [FromStr]
pub fn deserialize_max_notnan<'de, D, T>(deserializer: D) -> Result<Max<NotNan<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + Float,
    T::Err: Display,
{
    let value: T = display_fromstr::deserialize(deserializer)?;
    let value = NotNan::new(value).map_err(Error::custom)?;
    Ok(Max::with_initial(value))
}
