#![cfg_attr(feature = "cargo-clippy", allow(renamed_and_removed_lints))]

extern crate chrono;
extern crate serde;
extern crate serde_with;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_with::chrono::datetime_utc_ts_seconds_from_any;

#[cfg_attr(feature = "cargo-clippy", allow(large_enum_variant))]
#[serde(tag = "method", content = "params")]
#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub enum ChromeDebuggerMessage<S = String> {
    // Everything Network
    #[serde(
        rename = "Network.requestWillBeSent",
        rename_all = "camelCase"
    )]
    NetworkRequestWillBeSent {
        #[serde(rename = "documentURL")]
        document_url: S,
        request_id: S,
        request: Request<S>,
        initiator: Initiator<S>,
        redirect_response: Option<RedirectResponse<S>>,
        #[serde(deserialize_with = "datetime_utc_ts_seconds_from_any::deserialize")]
        wall_time: DateTime<Utc>,
    },
    #[serde(
        rename = "Network.requestServedFromCache",
        rename_all = "camelCase"
    )]
    NetworkRequestServedFromCache { request_id: S },
    #[serde(
        rename = "Network.responseReceived",
        rename_all = "camelCase"
    )]
    NetworkResponseReceived {
        request_id: S,
        response: Response<S>,
    },
    #[serde(
        rename = "Network.resourceChangedPriority",
        rename_all = "camelCase"
    )]
    NetworkResourceChangedPriority { request_id: S },
    #[serde(rename = "Network.loadingFailed", rename_all = "camelCase")]
    NetworkLoadingFailed { request_id: S },
    #[serde(rename = "Network.dataReceived", rename_all = "camelCase")]
    NetworkDataReceived { request_id: S },
    #[serde(rename = "Network.loadingFinished", rename_all = "camelCase")]
    NetworkLoadingFinished { request_id: S },
    #[serde(
        rename = "Network.webSocketCreated",
        rename_all = "camelCase"
    )]
    NetworkWebSocketCreated {
        request_id: S,
        url: S,
        initiator: Initiator<S>,
    },
    #[serde(rename = "Network.webSocketClosed", rename_all = "camelCase")]
    NetworkWebSocketClosed { request_id: S },
    #[serde(
        rename = "Network.webSocketWillSendHandshakeRequest",
        rename_all = "camelCase"
    )]
    NetworkWebSocketWillSendHandshakeRequest { request_id: S },
    #[serde(
        rename = "Network.webSocketHandshakeResponseReceived",
        rename_all = "camelCase"
    )]
    NetworkWebSocketHandshakeResponseReceived { request_id: S },
    #[serde(
        rename = "Network.webSocketFrameError",
        rename_all = "camelCase"
    )]
    NetworkWebSocketFrameError { request_id: S },
    #[serde(
        rename = "Network.webSocketFrameReceived",
        rename_all = "camelCase"
    )]
    NetworkWebSocketFrameReceived { request_id: S },
    #[serde(
        rename = "Network.webSocketFrameSent",
        rename_all = "camelCase"
    )]
    NetworkWebSocketFrameSent { request_id: S },
    #[serde(
        rename = "Network.eventSourceMessageReceived",
        rename_all = "camelCase"
    )]
    NetworkEventSourceMessageReceived { request_id: S },

    // Everything Target
    #[serde(rename = "Target.targetCreated", rename_all = "camelCase")]
    TargetTargetCreated {},
    #[serde(
        rename = "Target.targetInfoChanged",
        rename_all = "camelCase"
    )]
    TargetTargetInfoChanged { target_info: TargetInfo<S> },
    #[serde(rename = "Target.targetDestroyed", rename_all = "camelCase")]
    TargetTargetDestroyed {},
    #[serde(rename = "Target.attachedToTarget", rename_all = "camelCase")]
    TargetAttachedToTarget {},
    #[serde(
        rename = "Target.detachedFromTarget",
        rename_all = "camelCase"
    )]
    TargetDetachedFromTarget {},
    #[serde(
        rename = "Target.receivedMessageFromTarget",
        rename_all = "camelCase"
    )]
    TargetReceivedMessageFromTarget {},

    // Everything Debugger
    #[serde(rename = "Debugger.scriptParsed", rename_all = "camelCase")]
    DebuggerScriptParsed {
        script_id: S,
        url: S,
        stack_trace: Option<StackTrace<S>>,
    },
    #[serde(
        rename = "Debugger.scriptFailedToParse",
        rename_all = "camelCase"
    )]
    DebuggerScriptFailedToParse {
        script_id: S,
        url: S,
        stack_trace: Option<StackTrace<S>>,
    },
    #[serde(rename = "Debugger.paused", rename_all = "camelCase")]
    DebuggerPaused {},
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Request<S> {
    pub url: S,
    pub headers: Headers<S>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Headers<S> {
    #[serde(rename = "Referer")]
    pub referer: Option<S>,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct RedirectResponse<S> {
    pub url: S,
    pub timing: Timing,
}

/// Specifies a reason, why a network request happened
///
/// The variant `Initiator::Script` has different kinds in itself.
///
/// This is the typical case of XHR (XmlHttpRequests), which are caused by some JS code and
/// therefore have a stacktrace of where the XHR was caused.
///
/// ```json
/// {
///     "type": "script",
///     "stack": {
///         "callFrames": [
///             {
///                 "functionName": "Pe",
///                 "scriptId": "86",
///                 "url": "https://pagead2.googlesyndication.com/pagead/js/r20180917/r20180604/show_ads_impl.js",
///                 "lineNumber": 0,
///                 "columnNumber": 28644
///             }
///         ]
///     }
/// }
/// ```
///
/// This is the loading of a JS module. It looks similar to the parser case, because they are similar.
/// Just in this case the parser is parsing JS.
///
/// ```json
/// {
///     "type": "script",
///     "url": "http://example.com/",
///     "lineNumber": 2
/// }
///

#[serde(tag = "type", rename_all = "lowercase")]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum Initiator<S> {
    Other {},
    Parser { url: S },
    Script(InitiatorScript<S>),
}

#[serde(untagged)]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum InitiatorScript<S> {
    Stack { stack: StackTrace<S> },
    JsModule { url: S },
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StackTrace<S> {
    pub call_frames: Vec<CallFrame<S>>,
    pub parent: Option<Box<StackTrace<S>>>,
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct CallFrame<S> {
    pub url: S,
    pub script_id: S,
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct Response<S> {
    pub url: S,
    pub timing: Option<Timing>,
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct Timing {
    /// Start time of the request in seconds. All other times are relative to this one.
    #[serde(
        deserialize_with = "duration_seconds::deserialize",
        serialize_with = "duration_seconds_with_microseconds::serialize"
    )]
    pub request_time: Duration,
    /// Value in Milliseconds
    #[serde(
        deserialize_with = "duration_millis_opt::deserialize",
        serialize_with = "duration_seconds_with_microseconds_opt::serialize"
    )]
    pub dns_start: Option<Duration>,
    /// Value in Milliseconds
    #[serde(
        deserialize_with = "duration_millis_opt::deserialize",
        serialize_with = "duration_seconds_with_microseconds_opt::serialize"
    )]
    pub dns_end: Option<Duration>,
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, Serialize, Deserialize)]
pub struct TargetInfo<S> {
    pub url: S,
    #[serde(rename = "type")]
    pub target_type: TargetType,
}

#[serde(rename_all = "snake_case")]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, Serialize, Deserialize)]
pub enum TargetType {
    Page,
    BackgroundPage,
    Iframe,
    ServiceWorker,
    Worker,
    Browser,
    Other,
    Webview,
}

pub mod duration_millis_opt {
    use chrono::Duration;
    use serde::de::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        f64::deserialize(deserializer).map(|v| {
            if v < 0. {
                None
            } else {
                Some(Duration::nanoseconds((v * 1_000_000.) as i64))
            }
        })
    }
}

pub mod duration_seconds {
    use chrono::Duration;
    use serde::de::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        f64::deserialize(deserializer).map(|v| Duration::nanoseconds((v * 1_000_000_000.) as i64))
    }
}

pub mod duration_seconds_with_microseconds {
    use chrono::Duration;
    use serde::ser::Serializer;

    pub fn serialize<S>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(value.num_microseconds().unwrap() as f64 / 1_000_000.)
    }
}

pub mod duration_seconds_with_microseconds_opt {
    use chrono::Duration;
    use serde::ser::Serializer;

    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            None => serializer.serialize_f64(-1.),
            Some(v) => serializer.serialize_f64(v.num_microseconds().unwrap() as f64 / 1_000_000.),
        }
    }
}
