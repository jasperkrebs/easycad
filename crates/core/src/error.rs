/// Everything that can go wrong in the engine, as a message a person (or the agent) can act on.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    /// A setting is missing, not a number, or outside what the engine can handle.
    #[error("{name}: {reason}")]
    InvalidParam { name: &'static str, reason: String },
    /// A geometry operation failed (e.g. a curve could not be built).
    #[error("geometry: {0}")]
    Geometry(String),
    /// The result would be too large to compute or draw interactively.
    #[error("too large: {0}")]
    TooLarge(String),
}

/// Checks that `value` is a finite number within `min..=max`.
pub(crate) fn check_range(name: &'static str, value: f64, min: f64, max: f64) -> Result<(), Error> {
    if !value.is_finite() {
        return Err(Error::InvalidParam {
            name,
            reason: "must be a number".into(),
        });
    }
    if value < min || value > max {
        return Err(Error::InvalidParam {
            name,
            reason: format!("must be between {min} and {max}, got {value}"),
        });
    }
    Ok(())
}
