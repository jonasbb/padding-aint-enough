extern crate chrome;
extern crate failure;
extern crate min_max_heap;
extern crate sequences;
extern crate serde_json;

use chrome::ChromeDebuggerMessage;
use failure::{Error, Fail};
use min_max_heap::MinMaxHeap;
use sequences::common_sequence_classifications::{R008, R009};
use std::fmt::{self, Display};

pub fn take_largest<I, T>(iter: I, n: usize) -> Vec<T>
where
    I: IntoIterator<Item = T>,
    T: Ord,
{
    let mut iter = iter.into_iter();
    if n == 1 {
        // simply take the largest value and return it
        return iter.max().into_iter().collect();
    }

    let mut heap = MinMaxHeap::with_capacity(n);
    // fill the heap with n elements
    for _ in 0..n {
        match iter.next() {
            Some(v) => heap.push(v),
            None => break,
        }
    }

    // replace exisiting elements keeping the heap size
    for v in iter {
        heap.push_pop_min(v);
    }

    let res = heap.into_vec_desc();
    assert!(
        res.len() <= n,
        "Output vector only contains more than n elements."
    );
    res
}

/// A short-lived wrapper for some `Fail` type that displays it and all its
/// causes delimited by the string ": ".
pub struct DisplayCauses<'a>(&'a Fail);

impl<'a> Display for DisplayCauses<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)?;
        let mut x: &Fail = self.0;
        while let Some(cause) = x.cause() {
            f.write_str(": ")?;
            Display::fmt(&cause, f)?;
            x = cause;
        }
        Ok(())
    }
}

pub trait FailExt {
    fn display_causes(&self) -> DisplayCauses;
}

impl<T> FailExt for T
where
    T: Fail,
{
    fn display_causes(&self) -> DisplayCauses {
        DisplayCauses(self)
    }
}

pub trait ErrorExt {
    fn display_causes(&self) -> DisplayCauses;
}

impl ErrorExt for Error {
    fn display_causes(&self) -> DisplayCauses {
        DisplayCauses(self.as_fail())
    }
}

pub fn chrome_log_contains_errors<S>(msgs: &[ChromeDebuggerMessage<S>]) -> Option<&'static str>
where
    S: AsRef<str>,
{
    // test if there is a chrome error page
    let contains_chrome_error = msgs.iter().any(|msg| {
        if let ChromeDebuggerMessage::NetworkRequestWillBeSent { document_url, .. } = msg {
            document_url.as_ref() == "chrome-error://chromewebdata/"
        } else {
            false
        }
    });
    if contains_chrome_error {
        return Some(R008);
    }

    // Ensure at least one network request has succeeded.
    let contains_response_received = msgs.iter().any(|msg| {
        if let ChromeDebuggerMessage::NetworkResponseReceived { .. } = msg {
            true
        } else {
            false
        }
    });
    let contains_data_received = msgs.iter().any(|msg| {
        if let ChromeDebuggerMessage::NetworkDataReceived { .. } = msg {
            true
        } else {
            false
        }
    });
    if !(contains_response_received && contains_data_received) {
        return Some(R009);
    }

    // default case is `false`, meaning the data is good
    None
}
