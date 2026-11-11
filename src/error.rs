use std::error::Error;
use std::fmt::{Display, Formatter};

/// Error type returned by all API calls.
///
/// e-Stat reports errors in the `RESULT` element of the body, with a status
/// of `100` or more. [`Api`](Self::Api) holds that status, not the HTTP status.
///
/// # Example
///
/// ```
/// use estat_api::EstatError;
///
/// let error = EstatError::Api { status: 100, message: "認証に失敗しました。".into() };
/// assert!(error.is_unauthorized());
/// assert_eq!(error.status(), Some(100));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum EstatError {
    /// Failed to connect, send the request, or read the response body.
    ///
    /// The request URL is removed from the error because it contains the application ID.
    Connection(reqwest::Error),
    /// The response body is not the expected JSON.
    JsonParse(serde_json::Error),
    /// e-Stat returned an error in `RESULT`.
    ///
    /// `100` invalid application ID, `101` to `105` invalid parameters,
    /// `200` to `299` server errors, `300` no table or dataset with that ID.
    Api {
        /// `RESULT.STATUS`.
        status: u16,
        /// `RESULT.ERROR_MSG`, in Japanese unless English was requested.
        message: String,
    },
    /// The server answered with an HTTP error status and a body that is not
    /// an e-Stat response, such as a maintenance page. Holds the HTTP status and the body.
    HttpStatus(u16, String),
    /// The body is JSON but not shaped like an e-Stat response, or paging did
    /// not move forward.
    UnexpectedResponse(String),
    /// The request was rejected before sending.
    InvalidArgument(String),
}

impl EstatError {
    /// Status of [`Api`](Self::Api) (`RESULT.STATUS`) or
    /// [`HttpStatus`](Self::HttpStatus) (HTTP status), `None` otherwise.
    pub fn status(&self) -> Option<u16> {
        match self {
            EstatError::Api { status, .. } | EstatError::HttpStatus(status, _) => Some(*status),
            _ => None,
        }
    }

    /// `true` for status `100`: the application ID is wrong.
    pub fn is_unauthorized(&self) -> bool {
        self.api_status() == Some(100)
    }

    /// `true` for status `101` to `105`: a parameter is missing or invalid.
    pub fn is_invalid_parameter(&self) -> bool {
        self.api_status().is_some_and(|s| (101..=105).contains(&s))
    }

    /// `true` for status `300`: no table or dataset with the given ID.
    pub fn is_not_found(&self) -> bool {
        self.api_status() == Some(300)
    }

    /// `true` for status `200` to `299`: a temporary error on the e-Stat side.
    /// The manual asks to wait before trying again.
    pub fn is_server_error(&self) -> bool {
        self.api_status().is_some_and(|s| (200..300).contains(&s))
    }

    /// `true` when the request timed out.
    pub fn is_timeout(&self) -> bool {
        matches!(self, EstatError::Connection(e) if e.is_timeout())
    }

    fn api_status(&self) -> Option<u16> {
        match self {
            EstatError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub(crate) fn connection(error: reqwest::Error) -> Self {
        EstatError::Connection(error.without_url())
    }
}

impl Display for EstatError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EstatError::Connection(e) if e.is_timeout() => write!(f, "connection error: timed out"),
            EstatError::Connection(e) => write!(f, "connection error: {e}"),
            EstatError::JsonParse(e) => write!(f, "json parse error: {e}"),
            EstatError::Api { status, message } => write!(f, "e-stat error {status}: {message}"),
            EstatError::HttpStatus(status, body) => write!(f, "http status {status}: {body}"),
            EstatError::UnexpectedResponse(e) => write!(f, "unexpected response: {e}"),
            EstatError::InvalidArgument(e) => write!(f, "invalid argument: {e}"),
        }
    }
}

impl Error for EstatError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EstatError::Connection(e) => Some(e),
            EstatError::JsonParse(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(status: u16) -> EstatError {
        EstatError::Api {
            status,
            message: "message".into(),
        }
    }

    #[test]
    fn classifies_api_status() {
        assert!(api(100).is_unauthorized());
        assert!(api(101).is_invalid_parameter());
        assert!(api(105).is_invalid_parameter());
        assert!(!api(106).is_invalid_parameter());
        assert!(api(200).is_server_error());
        assert!(api(299).is_server_error());
        assert!(!api(300).is_server_error());
        assert!(api(300).is_not_found());
        assert_eq!(api(102).to_string(), "e-stat error 102: message");

        // HTTP statuses are not e-Stat statuses
        let error = EstatError::HttpStatus(300, "body".into());
        assert_eq!(error.status(), Some(300));
        assert!(!error.is_not_found());
        assert!(!error.is_invalid_parameter());
        assert!(!error.is_server_error());
    }

    #[test]
    fn connection_errors() {
        // A builder error is the only reqwest::Error that can be made without I/O
        let request = reqwest::Client::new().get("not a url").build();
        let error = EstatError::connection(request.unwrap_err());
        assert!(!error.is_timeout());
        assert!(error.to_string().starts_with("connection error: "));
        assert!(error.source().is_some());
        assert_eq!(error.status(), None);
    }

    #[test]
    fn display_and_source() {
        let json = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let error = EstatError::JsonParse(json);
        assert!(error.to_string().starts_with("json parse error: "));
        assert!(error.source().is_some());
        assert_eq!(error.status(), None);

        let error = EstatError::HttpStatus(503, "maintenance".into());
        assert_eq!(error.to_string(), "http status 503: maintenance");
        assert!(error.source().is_none());
        assert!(!error.is_timeout());
        assert!(!error.is_unauthorized());

        let error = EstatError::UnexpectedResponse("no RESULT".into());
        assert_eq!(error.to_string(), "unexpected response: no RESULT");
        let error = EstatError::InvalidArgument("bad".into());
        assert_eq!(error.to_string(), "invalid argument: bad");
    }
}
