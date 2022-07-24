#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Route error: {0}")]
    RouteError(#[from] route::Error),
    #[error("Incorrect destination: {0}")]
    IncorrectDestination(String),
}

/// For debugging purposes, the user may optionally set DEBUG_TRACE_AGENT=id for some agent id to
/// print a trace of all of that agent's actions. This function prints a log message for a
/// particular agent, if DEBUG_TRACE_AGENT is set and the traced id matches the specified id.
pub fn agent_log<F, S>(id: u64, msg: F)
where
    F: Fn() -> S,
    S: Into<String>,
{
    lazy_static::lazy_static! {
        static ref LOGGED_AGENT: Option<u64> = std::env::var("DEBUG_TRACE_AGENT").ok()
            .and_then(|val: String| val.parse().ok());
    }

    if let Some(logged_agent) = *LOGGED_AGENT {
        if logged_agent == id {
            println!("Log for agent {}: {}", id, msg().into());
        }
    }
}

pub fn agent_log_timestamp<F, S>(id: u64, msg: F, timestamp: u64)
where
    F: Fn() -> S,
    S: Into<String>,
{
    agent_log(id, || format!("{}: {}", timestamp, msg().into()));
}
