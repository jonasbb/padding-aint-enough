use chrono::{DateTime, Duration, Utc};
use serde_with::chrono::datetime_utc_ts_seconds_from_any;

#[cfg_attr(feature = "cargo-clippy", allow(large_enum_variant))]
#[serde(tag = "method", content = "params")]
#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub enum ChromeDebuggerMessage {
    // Everything Network
    #[serde(
        rename = "Network.requestWillBeSent",
        rename_all = "camelCase"
    )]
    NetworkRequestWillBeSent {
        #[serde(rename = "documentURL")]
        document_url: String,
        request_id: String,
        request: Request,
        initiator: Initiator,
        redirect_response: Option<RedirectResponse>,
        #[serde(deserialize_with = "datetime_utc_ts_seconds_from_any::deserialize")]
        wall_time: DateTime<Utc>,
    },
    #[serde(
        rename = "Network.requestServedFromCache",
        rename_all = "camelCase"
    )]
    NetworkRequestServedFromCache { request_id: String },
    #[serde(
        rename = "Network.responseReceived",
        rename_all = "camelCase"
    )]
    NetworkResponseReceived {
        request_id: String,
        response: Response,
    },
    #[serde(
        rename = "Network.resourceChangedPriority",
        rename_all = "camelCase"
    )]
    NetworkResourceChangedPriority { request_id: String },
    #[serde(rename = "Network.loadingFailed", rename_all = "camelCase")]
    NetworkLoadingFailed { request_id: String },
    #[serde(rename = "Network.dataReceived", rename_all = "camelCase")]
    NetworkDataReceived { request_id: String },
    #[serde(rename = "Network.loadingFinished", rename_all = "camelCase")]
    NetworkLoadingFinished { request_id: String },
    #[serde(
        rename = "Network.webSocketCreated",
        rename_all = "camelCase"
    )]
    NetworkWebSocketCreated {
        request_id: String,
        url: String,
        initiator: Initiator,
    },
    #[serde(rename = "Network.webSocketClosed", rename_all = "camelCase")]
    NetworkWebSocketClosed { request_id: String },
    #[serde(
        rename = "Network.webSocketWillSendHandshakeRequest",
        rename_all = "camelCase"
    )]
    NetworkWebSocketWillSendHandshakeRequest { request_id: String },
    #[serde(
        rename = "Network.webSocketHandshakeResponseReceived",
        rename_all = "camelCase"
    )]
    NetworkWebSocketHandshakeResponseReceived { request_id: String },
    #[serde(
        rename = "Network.webSocketFrameError",
        rename_all = "camelCase"
    )]
    NetworkWebSocketFrameError { request_id: String },
    #[serde(
        rename = "Network.webSocketFrameReceived",
        rename_all = "camelCase"
    )]
    NetworkWebSocketFrameReceived { request_id: String },
    #[serde(
        rename = "Network.webSocketFrameSent",
        rename_all = "camelCase"
    )]
    NetworkWebSocketFrameSent { request_id: String },

    // Everything Target
    #[serde(rename = "Target.targetCreated", rename_all = "camelCase")]
    TargetTargetCreated {},
    #[serde(
        rename = "Target.targetInfoChanged",
        rename_all = "camelCase"
    )]
    TargetTargetInfoChanged { target_info: TargetInfo },
    #[serde(rename = "Target.targetDestroyed", rename_all = "camelCase")]
    TargetTargetDestroyed {},
    #[serde(rename = "Target.attachedToTarget", rename_all = "camelCase")]
    TargetAttachedToTarget {},

    // Everything Debugger
    #[serde(rename = "Debugger.scriptParsed", rename_all = "camelCase")]
    DebuggerScriptParsed {
        script_id: String,
        url: String,
        stack_trace: Option<StackTrace>,
    },
    #[serde(
        rename = "Debugger.scriptFailedToParse",
        rename_all = "camelCase"
    )]
    DebuggerScriptFailedToParse {
        script_id: String,
        url: String,
        stack_trace: Option<StackTrace>,
    },
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Request {
    pub url: String,
    pub headers: Headers,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Headers {
    #[serde(rename = "Referer")]
    pub referer: Option<String>,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct RedirectResponse {
    pub url: String,
    pub timing: Timing,
}

#[serde(tag = "type", rename_all = "lowercase")]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum Initiator {
    Other {},
    Parser { url: String },
    Script { stack: StackTrace },
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StackTrace {
    pub call_frames: Vec<CallFrame>,
    pub parent: Option<Box<StackTrace>>,
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct CallFrame {
    pub url: String,
    pub script_id: String,
}

#[serde(rename_all = "camelCase")]
#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct Response {
    pub url: String,
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
pub struct TargetInfo {
    pub url: String,
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
