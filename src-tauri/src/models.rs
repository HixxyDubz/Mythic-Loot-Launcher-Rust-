use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub id: String,
    pub game: String,
    pub display_name: String,
    pub required_game_version: String,
    pub required_modpack_version: String,
    pub local_modpack_version: String,
    pub manifest_path: String,
    pub install_dir: String,
    pub game_dir: String,
    pub game_exe_path: String,
    pub launch_args: String,
    #[serde(default)]
    pub minecraft_launcher: String,
    pub discord_invite: String,
    pub update_source: String,
    pub manifest_url: String,
    pub deployment_subdir: String,
    pub logo_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherPreferences {
    pub reduce_motion: bool,
    pub auto_check_updates: bool,
    pub close_after_launch: bool,
}

impl Default for LauncherPreferences {
    fn default() -> Self {
        Self {
            reduce_motion: false,
            auto_check_updates: true,
            close_after_launch: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    pub schema_version: u32,
    pub selected_profile_id: String,
    pub profiles: Vec<GameProfile>,
    pub preferences: LauncherPreferences,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            schema_version: 2,
            selected_profile_id: "minecraft_main".into(),
            profiles: vec![minecraft_profile(), seven_days_profile()],
            preferences: LauncherPreferences::default(),
        }
    }
}

fn minecraft_profile() -> GameProfile {
    GameProfile {
        id: "minecraft_main".into(),
        game: "minecraft".into(),
        display_name: "Mythic Loot Minecraft".into(),
        required_game_version: "1.21.1".into(),
        required_modpack_version: "1.0.1".into(),
        local_modpack_version: String::new(),
        manifest_path: "manifests/minecraft_main.json".into(),
        install_dir: String::new(),
        game_dir: String::new(),
        game_exe_path: String::new(),
        launch_args: String::new(),
        minecraft_launcher: String::new(),
        discord_invite: String::new(),
        update_source: "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/download/v1.0.1/minecraft_main_1.0.1.zip".into(),
        manifest_url: "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/latest/download/minecraft_main-manifest.json".into(),
        deployment_subdir: String::new(),
        logo_path: "/assets/minecraft.png".into(),
    }
}

fn seven_days_profile() -> GameProfile {
    GameProfile {
        id: "seven_days_main".into(),
        game: "seven_days".into(),
        display_name: "Mythic Loot 7 Days".into(),
        required_game_version: "1.0".into(),
        required_modpack_version: "1.0.0".into(),
        local_modpack_version: String::new(),
        manifest_path: "manifests/seven_days_main.json".into(),
        install_dir: String::new(),
        game_dir: String::new(),
        game_exe_path: String::new(),
        launch_args: String::new(),
        minecraft_launcher: String::new(),
        discord_invite: String::new(),
        update_source: String::new(),
        manifest_url: "https://github.com/HixxyDubz/Mythic-Loot-7DTD-Modpack/releases/latest/download/seven_days_main-manifest.json".into(),
        deployment_subdir: "Mods".into(),
        logo_path: "/assets/seven-days.png".into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDefinition {
    pub id: String,
    pub display_name: String,
    pub detection_kind: String,
}

pub fn built_in_games() -> Vec<GameDefinition> {
    [
        ("minecraft", "Minecraft", "minecraft"),
        ("seven_days", "7 Days to Die", "steam"),
        ("palworld", "Palworld", "steam"),
        ("core_keeper", "Core Keeper", "steam"),
        ("marvel_heroes", "Marvel Heroes", "steam"),
        ("valheim", "Valheim", "steam"),
        ("factorio", "Factorio", "steam"),
        ("stardew_valley", "Stardew Valley", "steam"),
        ("hytale", "Hytale", "manual"),
        ("world_of_warcraft", "World of Warcraft", "manual"),
        ("runescape", "RuneScape", "manual"),
        ("city_of_heroes", "City of Heroes - Sanctuary", "manual"),
    ]
    .into_iter()
    .map(|(id, display_name, detection_kind)| GameDefinition {
        id: id.into(),
        display_name: display_name.into(),
        detection_kind: detection_kind.into(),
    })
    .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DetectedInstall {
    pub label: String,
    pub exe_path: Option<String>,
    pub install_dir: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReadinessStatus {
    Ready,
    UpdateRequired,
    RepairNeeded,
    GamePathMissing,
    SetupRequired,
    Checking,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileHealth {
    pub profile_id: String,
    pub status: ReadinessStatus,
    pub headline: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayload {
    pub config: LauncherConfig,
    pub games: Vec<GameDefinition>,
    pub health: Vec<ProfileHealth>,
    pub manifests: Vec<crate::manifest::ManifestSummary>,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOutcome {
    pub pid: u32,
    pub message: String,
}
