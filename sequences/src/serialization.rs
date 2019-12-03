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

pub mod serde_duration {
    use chrono::Duration;
    use serde::{
        de::{Deserializer, Error, Unexpected, Visitor},
        ser::Serializer,
    };

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;
        impl<'de> Visitor<'de> for Helper {
            type Value = Duration;

            fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                formatter.write_str("Invalid duration. Must be an integer, float, or string with optional subsecond precision.")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(Duration::seconds(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if value <= i64::max_value() as u64 {
                    Ok(Duration::seconds(value as i64))
                } else {
                    Err(Error::custom(format!(
                        "Invalid or out of range value '{}' for Duration",
                        value
                    )))
                }
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let seconds = value.trunc() as i64;
                let nsecs = (value.fract() * 1_000_000_000_f64).abs() as u32;
                Ok(Duration::seconds(seconds) + Duration::nanoseconds(i64::from(nsecs)))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let parts: Vec<_> = value.split('.').collect();

                match *parts.as_slice() {
                    [seconds] => {
                        if let Ok(seconds) = i64::from_str_radix(seconds, 10) {
                            Ok(Duration::seconds(seconds))
                        } else {
                            Err(Error::invalid_value(Unexpected::Str(value), &self))
                        }
                    }
                    [seconds, subseconds] => {
                        if let Ok(seconds) = i64::from_str_radix(seconds, 10) {
                            let subseclen = subseconds.chars().count() as u32;
                            if subseclen > 9 {
                                return Err(Error::custom(format!(
                                    "Duration only support nanosecond precision but '{}' has more than 9 digits.",
                                    value
                                )));
                            }

                            if let Ok(mut subseconds) = u32::from_str_radix(subseconds, 10) {
                                // convert subseconds to nanoseconds (10^-9), require 9 places for nanoseconds
                                subseconds *= 10u32.pow(9 - subseclen);
                                Ok(Duration::seconds(seconds)
                                    + Duration::nanoseconds(i64::from(subseconds)))
                            } else {
                                Err(Error::invalid_value(Unexpected::Str(value), &self))
                            }
                        } else {
                            Err(Error::invalid_value(Unexpected::Str(value), &self))
                        }
                    }

                    _ => Err(Error::invalid_value(Unexpected::Str(value), &self)),
                }
            }
        }

        deserializer.deserialize_any(Helper)
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let sec = d.num_seconds();
        let nsec = (*d - Duration::seconds(sec)).num_nanoseconds().unwrap();
        let s = format!("{}.{:>09}", sec, nsec);
        serializer.serialize_str(&*s)
    }
}
