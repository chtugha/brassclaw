#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentLogRecord {
    pub level: ComponentLogLevel,
    pub message: String,
}
