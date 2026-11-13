use crate::models::{Msg, Provider};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The provider answered with an error (or an error envelope).
    #[error("{message}")]
    Api { provider: Provider, status: u16, code: String, message: String, error_code: Option<i64> },
    /// The server could not be reached or the transfer broke (retryable).
    #[error("{0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
    #[error("{0}")]
    InvalidData(String),
    /// A user-facing, already localizable failure (the C# InvalidOperationException).
    #[error("{}", .0.render())]
    Operation(Msg),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("cancelled")]
    Cancelled,
}

impl Error {
    pub fn invalid(message: impl Into<String>) -> Self {
        Error::InvalidData(message.into())
    }
    pub fn operation(key: &str, args: &[&dyn ToString]) -> Self {
        Error::Operation(Msg::new(key, args))
    }
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Error::Cancelled)
    }
    /// The service answered that the torrent (or magnet) does not exist, e.g. it was deleted there.
    pub fn is_gone(&self) -> bool {
        matches!(self, Error::Api { status: 404, .. })
    }
    /// Transient failures worth replaying: connection loss, timeouts and Real-Debrid 5xx, 408 and 429 (it is just slow or rate limiting).
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Network(_) | Error::Timeout => true,
            Error::Api { provider: Provider::RealDebrid, status, .. } => *status >= 500 || matches!(*status, 408 | 429),
            _ => false,
        }
    }
    /// The message stored in a failed job, in the language-neutral form the UI translates.
    pub fn friendly(&self) -> Msg {
        match self {
            Error::Api { status: 401, .. } => Msg::plain("Token không hợp lệ hoặc hết hạn. Cập nhật token rồi thử lại."),
            Error::Api { provider: Provider::RealDebrid, status: 404, .. } => {
                Msg::plain("Torrent không còn trên dịch vụ. Bấm Thử lại để thêm lại.")
            }
            Error::Api { error_code: Some(21 | 33), .. } => {
                Msg::plain("Đã đạt giới hạn torrent đang hoạt động. Chờ torrent khác xong rồi thử lại.")
            }
            Error::Api { error_code: Some(23), .. } => Msg::plain("Tài khoản đã hết lưu lượng tải."),
            Error::Network(_) => Msg::plain("Lỗi kết nối hoặc link tải không khả dụng. Bấm Thử lại để tiếp tục."),
            Error::Timeout => Msg::plain("Kết nối quá thời gian chờ. Bấm Thử lại để tiếp tục."),
            Error::Operation(message) => message.clone(),
            other => Msg::plain(&other.to_string()),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        // Never keep the URL: TorBox download requests carry the API token in the query string.
        let error = error.without_url();
        if error.is_timeout() { Error::Timeout } else { Error::Network(error.to_string()) }
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::InvalidData(error.to_string())
    }
}
