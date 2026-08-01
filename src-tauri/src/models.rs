use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub remaining_percent: f64,
    pub resets_at: Option<String>,
    pub window_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    pub provider: String,
    pub display_name: String,
    pub plan: Option<String>,
    pub short_window: Option<UsageWindow>,
    pub weekly_window: Option<UsageWindow>,
    pub reset_credits: Option<u64>,
    pub reset_credit_expires_at: Vec<String>,
    pub updated_at: String,
    pub status: String,
    pub message: Option<String>,
}

impl ProviderSnapshot {
    pub fn failure(status: &str, message: &str) -> Self {
        Self {
            provider: "codex".into(),
            display_name: "CODEX".into(),
            plan: None,
            short_window: None,
            weekly_window: None,
            reset_credits: None,
            reset_credit_expires_at: Vec::new(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            status: status.into(),
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetPreferences {
    pub locked: bool,
    #[serde(default = "default_always_on_top")]
    pub always_on_top: bool,
    #[serde(default)]
    pub window_behavior_version: u8,
    #[serde(default)]
    pub stay_expanded: bool,
    pub pinned_provider: Option<String>,
    pub auto_rotate_seconds: u64,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_appearance")]
    pub appearance: String,
    #[serde(default = "default_skin")]
    pub selected_skin: String,
}

fn default_always_on_top() -> bool {
    true
}
fn default_language() -> String {
    "zh-CN".into()
}
fn default_appearance() -> String {
    "light".into()
}
fn default_skin() -> String {
    "default".into()
}

impl Default for WidgetPreferences {
    fn default() -> Self {
        Self {
            locked: false,
            always_on_top: true,
            window_behavior_version: 1,
            stay_expanded: false,
            pinned_provider: None,
            auto_rotate_seconds: 12,
            language: default_language(),
            appearance: default_appearance(),
            selected_skin: default_skin(),
        }
    }
}

impl WidgetPreferences {
    pub fn normalized(mut self) -> Self {
        if self.window_behavior_version < 1 {
            self.always_on_top = false;
            self.window_behavior_version = 1;
        }
        self.auto_rotate_seconds = self.auto_rotate_seconds.clamp(5, 300);
        if self.pinned_provider.as_deref() != Some("codex") {
            self.pinned_provider = None;
        }
        if self.language != "en" && self.language != "zh-CN" && self.language != "zh-TW" {
            self.language = default_language();
        }
        if self.appearance != "system" && self.appearance != "light" && self.appearance != "dark" {
            self.appearance = default_appearance();
        }
        if self.selected_skin != "default"
            && self.selected_skin != "blur"
            && self.selected_skin != "computer"
            && self.selected_skin != "mac-glass"
            && self.selected_skin != "tvos-focus"
            && self.selected_skin != "liquid-glass"
        {
            self.selected_skin = default_skin();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::WidgetPreferences;

    #[test]
    fn new_users_get_the_safe_minimal_defaults() {
        let preferences = WidgetPreferences::default();
        assert!(preferences.always_on_top);
        assert!(!preferences.locked);
        assert!(!preferences.stay_expanded);
        assert!(preferences.pinned_provider.is_none());
        assert_eq!(preferences.language, "zh-CN");
    }

    #[test]
    fn traditional_chinese_is_a_supported_language() {
        let mut preferences = WidgetPreferences::default();
        preferences.language = "zh-TW".into();
        assert_eq!(preferences.normalized().language, "zh-TW");
    }
}
