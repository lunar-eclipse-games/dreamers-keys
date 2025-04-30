#[derive(Debug, Clone)]
pub struct Character {
    pub account_id: u64,
    pub character_id: u32,
    pub name: String,
    pub kind: CharacterKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterKind {
    Normal,
    SoloAccount,
    SoloCharacter,
}
