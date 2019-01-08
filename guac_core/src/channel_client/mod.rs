macro_rules! forbidden {
    ($expression:expr, $label:expr) => {
        if !($expression) {
            return future::err(
                GuacError::Forbidden {
                    message: $label.to_string(),
                }
                .into(),
            );
        }
    };
}

pub mod channel;
pub mod channel_manager;
pub mod types;
