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

pub mod channel_manager;
pub mod combined_state;
pub mod types;

pub use self::types::Channel;
