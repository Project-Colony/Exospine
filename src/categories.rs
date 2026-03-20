//! Mail categories (labels/tags) with colors.
//!
//! Provides a predefined set of categories that can be applied to mail entries.
//! Each category has a name and a hex color string for UI display.

/// A mail category with a display color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    /// Category name (e.g. "Work", "Personal").
    pub name: String,
    /// Hex color string (e.g. "#3B82F6").
    pub color: String,
}

/// Return the predefined default categories.
pub fn default_categories() -> Vec<Category> {
    vec![
        Category {
            name: "Work".to_string(),
            color: "#3B82F6".to_string(), // blue
        },
        Category {
            name: "Personal".to_string(),
            color: "#22C55E".to_string(), // green
        },
        Category {
            name: "Important".to_string(),
            color: "#EF4444".to_string(), // red
        },
        Category {
            name: "Newsletter".to_string(),
            color: "#A855F7".to_string(), // purple
        },
        Category {
            name: "Social".to_string(),
            color: "#F97316".to_string(), // orange
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_categories_count() {
        let cats = default_categories();
        assert_eq!(cats.len(), 5);
        assert_eq!(cats[0].name, "Work");
        assert!(cats[0].color.starts_with('#'));
    }
}
