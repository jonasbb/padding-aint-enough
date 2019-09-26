//! Implementation of the [`SequenceElement`] type and associated traits

use crate::{constants::*, OneHotEncoding};
use serde::{self, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SequenceElement {
    Size(u8),
    Gap(u16),
}

impl SequenceElement {
    pub fn insert_cost(self) -> usize {
        use self::SequenceElement::*;

        debug_assert_ne!(self, Size(0), "Sequence contains a Size(0) elements");

        match self {
            // Size(0) => {
            //     // A size 0 packet should never occur
            //     error!("Sequence contains a Size(0) elements");
            //     usize::max_value()
            // }
            Size(_) => SIZE_INSERT_COST,
            Gap(g) => g as usize * GAP_INSERT_COST_MULTIPLIER,
        }
    }

    pub fn delete_cost(self) -> usize {
        // The delete costs have to be identical to the insert costs in order to be a metric.
        // There is no order in which two Sequences will be compared, so
        // xABCy -> xACy
        // must be the same as
        // xACy -> xABCy
        self.insert_cost()
    }

    pub fn substitute_cost(self, other: Self) -> usize {
        if self == other {
            return 0;
        }

        use self::SequenceElement::*;
        match (self, other) {
            // 2/3rds cost of insert
            (Size(_), Size(_)) => {
                (self.insert_cost() + other.delete_cost()) / SIZE_SUBSTITUTE_COST_DIVIDER
            }
            (Gap(g1), Gap(g2)) => {
                (g1.max(g2) - g1.min(g2)) as usize * GAP_SUBSTITUTE_COST_MULTIPLIER
            }
            (a, b) => a.delete_cost() + b.insert_cost(),
        }
    }

    pub fn swap_cost(self, other: Self) -> usize {
        if self == other {
            return 0;
        }

        SWAP_COST
    }

    pub fn to_one_hot_encoding(self) -> OneHotEncoding {
        use self::SequenceElement::*;
        let mut res = vec![0; 16];
        let len = res.len();
        match self {
            Size(0) => unreachable!(),
            Size(s) if s < len as u8 => res[s as usize] = 1,
            Gap(g) => res[0] = g,

            Size(s) => panic!("One Hot Encoding only works for Sequences not exceeding a Size({}), but found a Size({})", len - 1, s),
        }
        res
    }

    pub fn to_vector_encoding(self) -> (u16, u16) {
        use self::SequenceElement::*;
        match self {
            Size(s) => (u16::from(s), 0),
            Gap(g) => (0, g as u16),
        }
    }
}

impl Debug for SequenceElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::SequenceElement::*;
        let (l, v) = match self {
            Size(v) => ("S", u16::from(*v)),
            Gap(v) => ("G", *v),
        };
        write!(f, "{}{:>2}", l, v)
    }
}

impl Serialize for SequenceElement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let res = match self {
            SequenceElement::Gap(g) => format!("G{:0>2}", g),
            SequenceElement::Size(s) => format!("S{:0>2}", s),
        };
        serializer.serialize_str(&res)
    }
}

impl<'de> Deserialize<'de> for SequenceElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;
        use serde::de::Error;

        impl<'de> Visitor<'de> for Helper {
            type Value = SequenceElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "string in format `S00` or `G00`")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let chars = value.chars().count();
                if chars < 2 {
                    return Err(Error::custom(format!("The string must be at least 2 characters long (but got {}), in the format `S00` or `G00`.", chars)));
                }
                let start = value.chars().next().expect("String is 2 chars long.");
                match start {
                    'G' => {
                        let v = value[1..].parse::<u16>().map_err(|_| {
                            Error::custom(format!(
                                "The string must end in digits, but got `{:?}`.",
                                &value[1..]
                            ))
                        })?;
                        Ok(SequenceElement::Gap(v))
                    }
                    'S' => {
                        let v = value[1..].parse::<u8>().map_err(|_| {
                            Error::custom(format!(
                                "The string must end in digits, but got `{:?}`.",
                                &value[1..]
                            ))
                        })?;
                        Ok(SequenceElement::Size(v))
                    }
                    _ => Err(Error::custom(format!(
                        "The string must start with `G` or `S` but got `{}`.",
                        start
                    ))),
                }
            }
        }

        deserializer.deserialize_str(Helper)
    }
}

#[cfg(test)]
mod test {
    use super::SequenceElement::{self, *};
    #[test]
    fn test_serialize_elements() -> Result<(), serde_json::error::Error> {
        assert_eq!(&serde_json::to_string(&Gap(1))?, "\"G01\"");
        assert_eq!(&serde_json::to_string(&Gap(2))?, "\"G02\"");
        assert_eq!(&serde_json::to_string(&Gap(3))?, "\"G03\"");
        assert_eq!(&serde_json::to_string(&Gap(13))?, "\"G13\"");
        assert_eq!(&serde_json::to_string(&Gap(75))?, "\"G75\"");
        assert_eq!(&serde_json::to_string(&Gap(12345))?, "\"G12345\"");
        assert_eq!(
            &serde_json::to_string(&Gap(u16::max_value()))?,
            "\"G65535\""
        );

        assert_eq!(&serde_json::to_string(&Size(1))?, "\"S01\"");
        assert_eq!(&serde_json::to_string(&Size(2))?, "\"S02\"");
        assert_eq!(&serde_json::to_string(&Size(3))?, "\"S03\"");
        assert_eq!(&serde_json::to_string(&Size(4))?, "\"S04\"");
        assert_eq!(&serde_json::to_string(&Size(5))?, "\"S05\"");
        assert_eq!(&serde_json::to_string(&Size(10))?, "\"S10\"");
        assert_eq!(&serde_json::to_string(&Size(u8::max_value()))?, "\"S255\"");
        Ok(())
    }
    #[test]
    fn test_deserialize_elements() -> Result<(), serde_json::error::Error> {
        assert_eq!(serde_json::from_str::<SequenceElement>("\"G01\"")?, Gap(1));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"G02\"")?, Gap(2));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"G03\"")?, Gap(3));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"G13\"")?, Gap(13));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"G75\"")?, Gap(75));
        assert_eq!(
            serde_json::from_str::<SequenceElement>("\"G12345\"")?,
            Gap(12345)
        );
        assert_eq!(
            serde_json::from_str::<SequenceElement>("\"G65535\"")?,
            Gap(u16::max_value())
        );

        assert_eq!(serde_json::from_str::<SequenceElement>("\"S01\"")?, Size(1));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"S02\"")?, Size(2));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"S03\"")?, Size(3));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"S04\"")?, Size(4));
        assert_eq!(serde_json::from_str::<SequenceElement>("\"S05\"")?, Size(5));
        assert_eq!(
            serde_json::from_str::<SequenceElement>("\"S10\"")?,
            Size(10)
        );
        assert_eq!(
            serde_json::from_str::<SequenceElement>("\"S255\"")?,
            Size(u8::max_value())
        );
        Ok(())
    }
}
