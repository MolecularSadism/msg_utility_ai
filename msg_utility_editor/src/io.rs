use std::path::Path;

use msg_utility_ai::UtilityAIProfile;

/// Load a profile, choosing the format based on file extension.
/// `.json` files are loaded as JSON, everything else as RON.
pub fn load(path: &Path) -> Result<UtilityAIProfile, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("Failed to read: {e}"))?;
    if is_json(path) {
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse JSON: {e}"))
    } else {
        ron::from_str(&contents).map_err(|e| format!("Failed to parse RON: {e}"))
    }
}

/// Save a profile, choosing the format based on file extension.
/// `.json` files are saved as JSON, everything else as RON.
pub fn save(profile: &UtilityAIProfile, path: &Path) -> Result<(), String> {
    let contents = if is_json(path) {
        serde_json::to_string_pretty(profile)
            .map_err(|e| format!("Failed to serialize JSON: {e}"))?
    } else {
        let config = ron::ser::PrettyConfig::default()
            .depth_limit(10)
            .new_line(String::from("\n"))
            .indentor(String::from("    "));
        ron::ser::to_string_pretty(profile, config)
            .map_err(|e| format!("Failed to serialize RON: {e}"))?
    };
    std::fs::write(path, contents).map_err(|e| format!("Failed to write: {e}"))
}

fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "json")
}
