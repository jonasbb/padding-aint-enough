use misc_utils::{Max, Min};
use num_traits::float::Float;
use ordered_float::NotNan;
use serde::de::{Deserializer, Error, Visitor};
use std::{
    fmt::{self, Display},
    marker::PhantomData,
    str::FromStr,
};

/// Deserialize T using [FromStr]
pub fn deserialize_min_notnan<'de, D, T>(deserializer: D) -> Result<Min<NotNan<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + Float,
    T::Err: Display,
{
    Ok(Min::with_initial(
        deserializer.deserialize_str(NotNanHelper(PhantomData))?,
    ))
}

/// Deserialize T using [FromStr]
pub fn deserialize_max_notnan<'de, D, T>(deserializer: D) -> Result<Max<NotNan<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + Float,
    T::Err: Display,
{
    Ok(Max::with_initial(
        deserializer.deserialize_str(NotNanHelper(PhantomData))?,
    ))
}

struct NotNanHelper<S>(PhantomData<S>);

impl<'de, S> Visitor<'de> for NotNanHelper<S>
where
    S: FromStr + Float,
    <S as FromStr>::Err: Display,
{
    type Value = NotNan<S>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "valid json object")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let s: S = value.parse().map_err(Error::custom)?;
        NotNan::new(s).map_err(Error::custom)
    }
}
