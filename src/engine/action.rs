#[derive(Debug, Clone)]
pub enum Action {
    SetPreedit { text: String, begin: i32, end: i32 },
    CommitText(String),
    ClearPreedit,
    ForwardKey { key: u32, mods: u32 },
}
