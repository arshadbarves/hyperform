//! Skills: packaged instructions loaded on demand.

/// A loaded skill: its markdown body and the directory its relative paths
/// refer to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillContent {
    pub body: String,
    pub dir: String,
}

pub trait SkillPort: Send + Sync {
    /// `(name, description)` of every available skill.
    fn list(&self) -> Vec<(String, String)>;

    fn load(&self, name: &str) -> Result<SkillContent, String>;
}
