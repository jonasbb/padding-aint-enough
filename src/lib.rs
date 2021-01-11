use chrome::ChromeDebuggerMessage;
use min_max_heap::MinMaxHeap;
use sequences::common_sequence_classifications::{R008, R009};

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
    let contains_response_received = msgs
        .iter()
        .any(|msg| matches!(msg, ChromeDebuggerMessage::NetworkResponseReceived { .. }));
    let contains_data_received = msgs
        .iter()
        .any(|msg| matches!(msg, ChromeDebuggerMessage::NetworkDataReceived { .. }));
    if !(contains_response_received && contains_data_received) {
        return Some(R009);
    }

    // default case is `false`, meaning the data is good
    None
}
