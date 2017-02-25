//! slog logging macros
#[doc(hidden)]
#[macro_export]
macro_rules! try_trace(
    ($l:expr, $($k:expr => $v:expr),+; $($args:tt)+) => {
        if let Some(ref log) = $l {
            trace!(log, $($k => $v),+; $($args)+);
        }
    };
    ($l:expr, $($args:tt)+) => {
        if let Some(ref log) = $l {
            trace!(log, $($args)+);
        }
    }
);

#[doc(hidden)]
#[macro_export]
macro_rules! try_debug(
    ($l:expr, $($k:expr => $v:expr),+; $($args:tt)+) => {
        if let Some(ref log) = $l {
            debug!(log, $($k => $v),+; $($args)+);
        }
    };
    ($l:expr, $($args:tt)+) => {
        if let Some(ref log) = $l {
            debug!(log, $($args)+);
        }
    }
);

#[doc(hidden)]
#[macro_export]
macro_rules! try_info(
    ($l:expr, $($k:expr => $v:expr),+; $($args:tt)+) => {
        if let Some(ref log) = $l {
            info!(log, $($k => $v),+; $($args)+);
        }
    };
    ($l:expr, $($args:tt)+) => {
        if let Some(ref log) = $l {
            info!(log, $($args)+);
        }
    }
);

#[doc(hidden)]
#[macro_export]
macro_rules! try_warn(
    ($l:expr, $($k:expr => $v:expr),+; $($args:tt)+) => {
        if let Some(ref log) = $l {
            warn!(log, $($k => $v),+; $($args)+);
        }
    };
    ($l:expr, $($args:tt)+) => {
        if let Some(ref log) = $l {
            warn!(log, $($args)+);
        }
    }
);

#[doc(hidden)]
#[macro_export]
macro_rules! try_error(
    ($l:expr, $($k:expr => $v:expr),+; $($args:tt)+) => {
        if let Some(ref log) = $l {
            error!(log, $($k => $v),+; $($args)+);
        }
    };
    ($l:expr, $($args:tt)+) => {
        if let Some(ref log) = $l {
            error!(log, $($args)+);
        }
    }
);

#[doc(hidden)]
#[macro_export]
macro_rules! try_crit(
    ($l:expr, $($k:expr => $v:expr),+; $($args:tt)+) => {
        if let Some(ref log) = $l {
            crit!(log, $($k => $v),+; $($args)+);
        }
    };
    ($l:expr, $($args:tt)+) => {
        if let Some(ref log) = $l {
            crit!(log, $($args)+);
        }
    }
);
