//! slog logging macros
macro_rules! stdout_trace(
    ($($k:expr => $v:expr),+; $($args:tt)+) => {
        trace!($crate::util::STDOUT, $($k => $v),+; $($args)+)
    };
    ($($args:tt)+) => {
        trace!($crate::util::STDOUT, $($args)+)
    };
);

macro_rules! stderr_trace(
    ($($k:expr => $v:expr),+; $($args:tt)+) => {
        trace!($crate::util::STDERR, $($k => $v),+; $($args)+)
    };
    ($($args:tt)+) => {
        trace!($crate::util::STDERR, $($args)+)
    };
);

macro_rules! stdout_warn(
    ($($k:expr => $v:expr),+; $($args:tt)+) => {
        warn!($crate::util::STDOUT, $($k => $v),+; $($args)+)
    };
    ($($args:tt)+) => {
        warn!($crate::util::STDOUT, $($args)+)
    };
);

macro_rules! stdout_error(
    ($($k:expr => $v:expr),+; $($args:tt)+) => {
        error!($crate::util::STDOUT, $($k => $v),+; $($args)+)
    };
    ($($args:tt)+) => {
        error!($crate::util::STDOUT, $($args)+)
    };
);

macro_rules! stderr_error(
    ($($k:expr => $v:expr),+; $($args:tt)+) => {
        error!($crate::util::STDERR, $($k => $v),+; $($args)+)
    };
    ($($args:tt)+) => {
        error!($crate::util::STDERR, $($args)+)
    };
);
