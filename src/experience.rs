use crate::{
    appearance::{Appearance, Background, Texture},
    soundscape::Settings,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Atmosphere {
    pub name: String,
    pub appearance: Appearance,
    pub sound: Settings,
}
pub fn atmospheres() -> Vec<Atmosphere> {
    [
        (
            "Rainy evening",
            true,
            [19, 30, 41],
            [43, 62, 82],
            Texture::Grain,
            1,
        ),
        (
            "Forest morning",
            false,
            [221, 235, 213],
            [194, 216, 190],
            Texture::Linen,
            2,
        ),
        (
            "Ocean calm",
            false,
            [218, 237, 244],
            [187, 214, 230],
            Texture::None,
            4,
        ),
    ]
    .into_iter()
    .map(|(name, dark, color, second, texture, preset)| {
        let mut sound = Settings::default();
        sound.choose(preset);
        Atmosphere {
            name: name.into(),
            appearance: Appearance {
                dark,
                color,
                second,
                texture,
                background: Background::Gradient,
            },
            sound,
        }
    })
    .collect()
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Day {
    pub sessions: u64,
    pub focus_secs: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Experience {
    pub intention: String,
    pub focus_view: bool,
    pub atmospheres: Vec<Atmosphere>,
    pub days: BTreeMap<String, Day>,
}
impl Default for Experience {
    fn default() -> Self {
        Self {
            intention: String::new(),
            focus_view: false,
            atmospheres: atmospheres(),
            days: BTreeMap::new(),
        }
    }
}
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
impl Experience {
    pub fn record(&mut self, date: String, sessions: u64, seconds: u64) {
        let day = self.days.entry(date).or_default();
        day.sessions = day.sessions.saturating_add(sessions);
        day.focus_secs = day.focus_secs.saturating_add(seconds);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_totals_accumulate_and_dates_stay_separate() {
        let mut prefs = Experience::default();
        prefs.record("2026-10-06".into(), 1, 1500);
        prefs.record("2026-10-06".into(), 2, 3000);
        prefs.record("2026-10-07".into(), 1, 1200);
        assert_eq!(
            prefs.days["2026-10-06"],
            Day {
                sessions: 3,
                focus_secs: 4500
            }
        );
        let saved = serde_json::to_string(&prefs).unwrap();
        assert_eq!(prefs, serde_json::from_str(&saved).unwrap());
    }
}
