use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct KeyboardState {
    pressed: HashMap<glfw::Key, glfw::Modifiers>,
    just_pressed: HashMap<glfw::Key, glfw::Modifiers>,
    just_released: HashMap<glfw::Key, glfw::Modifiers>,
}

impl KeyboardState {
    pub fn is_pressed(&self, key: glfw::Key, mods: Option<glfw::Modifiers>) -> bool {
        let Some(pressed) = self.pressed.get(&key) else {
            return false;
        };

        if let Some(mods) = mods {
            *pressed == mods
        } else {
            true
        }
    }

    pub fn is_just_pressed(&self, key: glfw::Key, mods: Option<glfw::Modifiers>) -> bool {
        let Some(just_pressed) = self.just_pressed.get(&key) else {
            return false;
        };

        if let Some(mods) = mods {
            *just_pressed == mods
        } else {
            true
        }
    }

    pub fn is_just_released(&self, key: glfw::Key, mods: Option<glfw::Modifiers>) -> bool {
        let Some(just_released) = self.just_released.get(&key) else {
            return false;
        };

        if let Some(mods) = mods {
            *just_released == mods
        } else {
            true
        }
    }

    pub fn press(&mut self, key: glfw::Key, mods: glfw::Modifiers) {
        self.pressed.insert(key, mods);
        self.just_pressed.insert(key, mods);
    }

    pub fn release(&mut self, key: glfw::Key, mods: glfw::Modifiers) {
        self.just_released.insert(key, mods);
    }

    pub fn post_update(&mut self) {
        self.just_pressed.clear();
        self.clear_released();
        self.just_released.clear();
    }

    fn clear_released(&mut self) {
        self.pressed
            .retain(|k, _| !self.just_released.contains_key(k));
    }
}
