use lurklet_reddit::TransportErrorKind;

pub fn transport_kind(kind: TransportErrorKind) -> &'static str {
    match kind {
        TransportErrorKind::Timeout => "timeout",
        TransportErrorKind::Connect => "connect",
        TransportErrorKind::Body => "body",
        TransportErrorKind::Request => "request",
        TransportErrorKind::Initialization => "initialization",
        TransportErrorKind::Other => "other",
        _ => "unknown",
    }
}
