use crate::{
    precision_sequence::PrecisionSequence, AbstractQueryResponse, Sequence, SequenceElement,
};
use chrono::Duration;
use failure::{bail, Error};
use std::str::FromStr;

/// Specifies how to load data into a [`Sequence`] and which processing steps to perform
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct LoadSequenceConfig {
    pub padding: Padding,
    pub gap_mode: GapMode,
    pub simulated_countermeasure: SimulatedCountermeasure,
}

/// Specify padding strategy to use
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Padding {
    ///  \[DEFAULT\]
    Q128R468,
}

impl Default for Padding {
    fn default() -> Self {
        Self::Q128R468
    }
}

impl FromStr for Padding {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Q128R468" | "q128r468" => Ok(Self::Q128R468),
            unkwn => bail!("Unknown variant: '{}'", unkwn),
        }
    }
}

/// Specifies how time should be converted into gaps
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum GapMode {
    /// Convert time based on the log2 function \[DEFAULT\]
    Log2,
    /// Use the identity function
    Ident,
}

impl Default for GapMode {
    fn default() -> Self {
        Self::Log2
    }
}

impl FromStr for GapMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Log2" | "log2" => Ok(Self::Log2),
            "Ident" | "ident" => Ok(Self::Ident),
            unkwn => bail!("Unknown variant: '{}'", unkwn),
        }
    }
}

/// Simulate different countermeasures while loading the [Sequence] data
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum SimulatedCountermeasure {
    /// Do not apply any post-processing steps
    None,
    /// Assume perfect padding is applied.
    ///
    /// This removes all [`SequenceElement::Size`] from the [`Sequence`].
    PerfectPadding,
    /// Assume perfect timing defense
    ///
    /// This removes all [`SequenceElement::Gap`] from the [`Sequence`].
    PerfectTiming,
}

impl Default for SimulatedCountermeasure {
    fn default() -> Self {
        Self::None
    }
}

/// Takes a list of Queries and returns a [`Sequence`]
///
/// The functions abstracts over some details of Queries, such as absolute size and absolute time.
/// The function only returns [`None`], if the input sequence is empty.
pub fn convert_to_sequence<QR>(
    data: impl IntoIterator<Item = QR>,
    identifier: String,
    config: LoadSequenceConfig,
) -> Option<Sequence>
where
    QR: Into<AbstractQueryResponse>,
{
    let base_gap_size = Duration::microseconds(1000);

    let mut last_time = None;
    let data: Vec<_> = data
        .into_iter()
        .flat_map(|d| {
            let d: AbstractQueryResponse = d.into();

            let mut gap = None;
            if let Some(last_end) = last_time {
                gap = gap_size(d.time - last_end, base_gap_size, config.gap_mode);
            }

            let mut size = Some(pad_size(d.size, false, config.padding));

            // The config allows us to remove either Gap or Size
            match config.simulated_countermeasure {
                SimulatedCountermeasure::None => {}
                SimulatedCountermeasure::PerfectPadding => {
                    // We need to enforce Gap(0) messages to ensure that counting the number of messages still works

                    // If `last_end` is set, then there was a previous message, so we need to add a gap
                    // Only add a gap, if there is not one already
                    if last_time.is_some() && gap.is_none() {
                        gap = Some(SequenceElement::Gap(0));
                    }
                    size = None;
                }
                SimulatedCountermeasure::PerfectTiming => {
                    gap = None;
                }
            }

            // Mark this as being not the first iteration anymore
            last_time = Some(d.time);

            gap.into_iter().chain(size)
        })
        .collect();

    if data.is_empty() {
        return None;
    }

    Some(Sequence::new(data, identifier))
}

/// Takes a list of Queries and returns a [`PrecisionSequence`]
///
/// The functions abstracts over some details of Querys, such as absolute size and absolute time.
/// The function only returns [`None`], if the input sequence is empty.
pub fn convert_to_precision_sequence<QR>(
    data: impl IntoIterator<Item = QR>,
    identifier: String,
) -> Option<PrecisionSequence>
where
    QR: Into<AbstractQueryResponse>,
{
    let data: Vec<_> = data.into_iter().map(Into::into).collect();
    if data.is_empty() {
        return None;
    }

    Some(PrecisionSequence::new(data, identifier))
}

pub(crate) fn gap_size(gap: Duration, base: Duration, mode: GapMode) -> Option<SequenceElement> {
    if gap <= base {
        return None;
    }
    let mut gap = gap;
    let mut out = 0;
    while gap > base {
        gap = gap - base;
        out += 1;
    }

    let dist = match mode {
        GapMode::Log2 => f64::from(out).log2() as _,
        GapMode::Ident => out as _,
    };

    // // FIXME: Shift Gap values to better align the Pi data with the server data
    // let dist = match dist {
    //     x @ 0..=1 => x,
    //     2..=4 => 2,
    //     x => x-2,
    // };

    if dist == 0 {
        None
    } else {
        Some(SequenceElement::Gap(dist))
    }
}

pub(crate) fn pad_size(size: u32, is_query: bool, padding: Padding) -> SequenceElement {
    use self::Padding::*;
    SequenceElement::Size(match (padding, is_query) {
        (Q128R468, true) => block_padding(size, 128) / 128,
        (Q128R468, false) => block_padding(size, 468) / 468,
    } as u8)
}

fn block_padding(size: u32, block_size: u32) -> u32 {
    if size % block_size == 0 {
        size
    } else {
        size / block_size * block_size + block_size
    }
}

#[test]
fn test_block_padding() {
    assert_eq!(0, block_padding(0, 128));
    assert_eq!(128, block_padding(1, 128));
    assert_eq!(128, block_padding(127, 128));
    assert_eq!(128, block_padding(128, 128));
    assert_eq!(128 * 2, block_padding(129, 128));
}
