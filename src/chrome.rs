use chrono::{DateTime, Utc};
use serde_with::chrono::datetime_utc_ts_seconds_from_any;

#[serde(tag = "method", content = "params")]
#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub enum ChromeDebuggerMessage {
    // Everything Network
    #[serde(rename = "Network.requestWillBeSent", rename_all = "camelCase")]
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
    #[serde(rename = "Network.requestServedFromCache", rename_all = "camelCase")]
    NetworkRequestServedFromCache { request_id: String },
    #[serde(rename = "Network.responseReceived", rename_all = "camelCase")]
    NetworkResponseReceived {
        request_id: String,
        response: Response,
    },
    #[serde(rename = "Network.resourceChangedPriority", rename_all = "camelCase")]
    NetworkResourceChangedPriority { request_id: String },
    #[serde(rename = "Network.loadingFailed", rename_all = "camelCase")]
    NetworkLoadingFailed { request_id: String },
    #[serde(rename = "Network.dataReceived", rename_all = "camelCase")]
    NetworkDataReceived { request_id: String },
    #[serde(rename = "Network.loadingFinished", rename_all = "camelCase")]
    NetworkLoadingFinished { request_id: String },

    // Everything Target
    #[serde(rename = "Target.targetCreated", rename_all = "camelCase")]
    TargetTargetCreated {},
    #[serde(rename = "Target.targetInfoChanged", rename_all = "camelCase")]
    TargetTargetInfoChanged {},
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
    #[serde(rename = "Debugger.scriptFailedToParse", rename_all = "camelCase")]
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

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RedirectResponse {
    pub url: String,
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
    /// Start time of the request. All other times are relative to this one
    pub request_time: f64,
    /// Value in Milliseconds
    #[serde(deserialize_with = "negative_is_none::deserialize")]
    pub dns_start: Option<f64>,
    /// Value in Milliseconds
    #[serde(deserialize_with = "negative_is_none::deserialize")]
    pub dns_end: Option<f64>,
}

pub mod negative_is_none {
    use num_traits;
    use serde::de::{Deserialize, Deserializer};

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        T: Deserialize<'de> + num_traits::Signed + num_traits::One,
        D: Deserializer<'de>,
    {
        let v = T::deserialize(deserializer)?;
        Ok(if v.is_negative() && v.abs().is_one() {
            None
        } else {
            Some(v)
        })
    }
}
