pub mod chat;
pub mod messages;
pub mod responses;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireMode {
    ChatCompletions,
    Messages,
    Responses,
}
