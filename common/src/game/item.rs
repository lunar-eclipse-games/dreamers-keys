use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum ItemExemplar {
    Individual { item: Item },
    Stackable { stackable_id: String, amount: usize },
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub base_id: String,
    pub category: ItemCategory,
    pub rarity: Rarity,
    pub implicits: Vec<Modifier>,
    pub explicits: Vec<Modifier>,
    pub condition: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemCategory {
    Sword,
    Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    /// Gray (0 mods)
    Insignificant,
    /// Blue (3 mods)
    Fabled,
    /// Purple (5 mods)
    Legendary,
    /// Red (7 mods)
    Epic,
    /// Orange (unique)
    Mythic,
    // Future perhaps Yellow?
}

impl Rarity {
    pub fn maximum_explicit_count(self) -> usize {
        match self {
            Rarity::Insignificant => 0,
            Rarity::Fabled => 3,
            Rarity::Legendary => 5,
            Rarity::Epic => 7,
            Rarity::Mythic => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Modifier {
    pub modifier_id: String,
    pub rolls: Vec<i32>,
}
