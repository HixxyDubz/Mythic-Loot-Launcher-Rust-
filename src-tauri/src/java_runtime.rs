//! Read-only discovery. Never executes a discovered binary or installs Java.
use crate::{launch, models::GameProfile, operations::WorkGuard, safe_path};
use serde::Serialize;
use std::{
    collections::HashSet,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    executable: String,
    version: String,
    vendor: String,
    architecture: String,
    source: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaDiscovery {
    runtimes: Vec<JavaRuntime>,
    limited: bool,
}

#[tauri::command]
pub async fn detect_java_runtimes() -> Result<JavaDiscovery, String> {
    let guard = WorkGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        discover()
    })
    .await
    .map_err(|error| format!("Java discovery failed: {error}"))
}

fn discover() -> JavaDiscovery {
    let mut roots: Vec<(PathBuf, &str, usize)> = Vec::new();
    if let Some(home) = env::var_os("JAVA_HOME").filter(|value| !value.is_empty()) {
        roots.push((PathBuf::from(home).join("bin"), "JAVA_HOME", 1));
    }
    if let Some(paths) = env::var_os("PATH") {
        roots.extend(
            env::split_paths(&paths)
                .filter(|path| path.is_absolute())
                .map(|path| (path, "PATH", 1)),
        );
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env::var_os(variable).filter(|value| !value.is_empty()) {
            for vendor in [
                "Eclipse Adoptium",
                "Java",
                "Microsoft",
                "Amazon Corretto",
                "Zulu",
                "BellSoft",
                "Semeru",
                "Oracle",
                "AdoptOpenJDK",
            ] {
                roots.push((PathBuf::from(&root).join(vendor), vendor, 6));
            }
        }
    }
    for (variable, suffix, source) in [
        ("APPDATA", ".minecraft/runtime", "Minecraft runtime"),
        (
            "LOCALAPPDATA",
            "Programs/Minecraft Launcher/runtime",
            "Minecraft runtime",
        ),
        (
            "USERPROFILE",
            "curseforge/minecraft/Install/runtime",
            "CurseForge runtime",
        ),
        (
            "APPDATA",
            "ModrinthApp/meta/java_versions",
            "Modrinth runtime",
        ),
    ] {
        if let Some(root) = env::var_os(variable).filter(|value| !value.is_empty()) {
            roots.push((PathBuf::from(root).join(suffix), source, 8));
        }
    }
    discover_roots(&roots)
}

fn discover_roots(roots: &[(PathBuf, &str, usize)]) -> JavaDiscovery {
    let mut result = JavaDiscovery::default();
    let mut seen = HashSet::new();
    let started = Instant::now();
    let mut visited = 0;
    for (root, source, depth) in roots {
        if !root.is_absolute() || !root.is_dir() || safe_path::reject_link_path(root).is_err() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .max_depth(*depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| safe_path::reject_link_path(entry.path()).is_ok())
        {
            visited += 1;
            if visited > 20_000 || started.elapsed() > Duration::from_secs(5) {
                result.limited = true;
                return result;
            }
            let Ok(entry) = entry else {
                continue;
            };
            if !entry.file_type().is_file() || !entry.file_name().eq_ignore_ascii_case("java.exe") {
                continue;
            }
            let Ok(path) = fs::canonicalize(entry.path()) else {
                continue;
            };
            if !seen.insert(path.to_string_lossy().to_lowercase()) {
                continue;
            }
            result.runtimes.push(inspect(&path, source));
        }
    }
    result
        .runtimes
        .sort_by(|a, b| a.executable.cmp(&b.executable));
    result
}

fn inspect(path: &Path, source: &str) -> JavaRuntime {
    // The JDK's release file is data, not proof of binary identity. Surface
    // only these three fields, never unrelated environment/account values.
    let release = path
        .parent()
        .and_then(Path::parent)
        .map(|home| home.join("release"));
    let contents = release
        .and_then(|path| {
            safe_path::reject_link_path(&path).ok()?;
            let file = fs::File::open(path).ok()?;
            let mut data = String::new();
            file.take(65_537).read_to_string(&mut data).ok()?;
            (data.len() <= 65_536).then_some(data)
        })
        .unwrap_or_default();
    let field = |name: &str| {
        contents
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once('=')?;
                (key == name).then(|| {
                    value
                        .trim()
                        .trim_matches('"')
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(120)
                        .collect::<String>()
                })
            })
            .unwrap_or_default()
    };
    JavaRuntime {
        executable: path.display().to_string(),
        version: field("JAVA_VERSION"),
        vendor: field("IMPLEMENTOR"),
        architecture: field("OS_ARCH"),
        source: source.into(),
    }
}

pub fn is_java_executable(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        name.eq_ignore_ascii_case("java.exe") || name.eq_ignore_ascii_case("javaw.exe")
    })
}

#[tauri::command]
pub fn prepare_java_arguments(
    profile: GameProfile,
    memory_mb: Option<u32>,
) -> Result<String, String> {
    let _guard = WorkGuard::begin()?;
    if profile.game != "minecraft"
        || !profile.minecraft_launcher.is_empty()
        || !is_java_executable(Path::new(profile.game_exe_path.trim()))
    {
        return Err("Memory arguments can only be applied to a direct Java launch. Configure CurseForge or Modrinth in that launcher's settings.".into());
    }
    let path = Path::new(profile.game_exe_path.trim());
    safe_path::reject_link_path(path)?;
    if !path.is_absolute() || !path.is_file() {
        return Err("Choose an existing Java executable first".into());
    }
    memory_arguments(&profile.launch_args, memory_mb)
}

fn memory_arguments(input: &str, memory_mb: Option<u32>) -> Result<String, String> {
    if memory_mb.is_some_and(|mb| !(512..=65_536).contains(&mb)) {
        return Err(
            "Choose between 512 and 65536 MiB. Leave enough memory for Windows and other apps."
                .into(),
        );
    }
    let args = launch::split_windows_args(input)?;
    if args.iter().any(|arg| arg.starts_with('@')) {
        return Err("Memory editing does not expand Java @argument files. Edit their memory settings directly.".into());
    }
    let mut prefix = Vec::new();
    let mut index = 0;
    // Stop at the Java entry point. Application arguments after it must never
    // be removed, even if they happen to look like JVM memory flags.
    while index < args.len() {
        let arg = &args[index];
        if !arg.starts_with('-')
            || matches!(arg.as_str(), "-jar" | "-m" | "--module" | "--")
            || arg.starts_with("--module=")
        {
            break;
        }
        if arg == "-Xms" || arg == "-Xmx" {
            return Err(
                "Use attached memory values such as -Xmx4096M before editing memory.".into(),
            );
        }
        if arg.starts_with("-Xms")
            || arg.starts_with("-Xmx")
            || arg.starts_with("-XX:InitialHeapSize=")
            || arg.starts_with("-XX:MaxHeapSize=")
        {
            index += 1;
            continue;
        }
        prefix.push(arg.clone());
        if matches!(
            arg.as_str(),
            "-cp"
                | "-classpath"
                | "--class-path"
                | "-p"
                | "--module-path"
                | "--upgrade-module-path"
                | "--add-modules"
                | "--limit-modules"
                | "--add-reads"
                | "--add-exports"
                | "--add-opens"
                | "--patch-module"
                | "--enable-native-access"
                | "--source"
                | "--describe-module"
                | "-d"
        ) {
            index += 1;
            prefix.push(
                args.get(index)
                    .ok_or("A Java option is missing its value")?
                    .clone(),
            );
        }
        index += 1;
    }
    if index == args.len() {
        return Err("Configure the Java main class or -jar launch target first. Mythic Loot does not build Minecraft authentication or library arguments.".into());
    }
    if args[index].is_empty()
        || args[index] == "--module="
        || (matches!(args[index].as_str(), "-jar" | "-m" | "--module" | "--")
            && args
                .get(index + 1)
                .is_none_or(|target| target.is_empty() || target.starts_with('-')))
    {
        return Err("The Java launch target is missing".into());
    }
    if let Some(mb) = memory_mb {
        prefix.push(format!("-Xms{}M", mb / 2));
        prefix.push(format!("-Xmx{mb}M"));
    }
    prefix.extend_from_slice(&args[index..]);
    Ok(prefix
        .iter()
        .map(|arg| quote_argument(arg))
        .collect::<Vec<_>>()
        .join(" "))
}

fn quote_argument(arg: &str) -> String {
    // Quote every token using Windows escaping, including empty arguments and
    // paths ending in backslashes. split_windows_args is the inverse.
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in arg.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        result.extend(std::iter::repeat_n(
            '\\',
            if ch == '"' { slashes * 2 + 1 } else { slashes },
        ));
        slashes = 0;
        result.push(ch);
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_reads_only_real_release_fields_and_deduplicates() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("jdk/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("java.exe"), b"not executed by discovery").unwrap();
        fs::write(root.path().join("jdk/release"), "JAVA_VERSION=\"21.0.1\"\nIMPLEMENTOR=\"Test JDK\"\nOS_ARCH=\"amd64\"\nPRIVATE_FIELD=never-return").unwrap();
        let found = discover_roots(&[(root.path().to_owned(), "test", 4), (bin, "duplicate", 1)]);
        assert_eq!(found.runtimes.len(), 1);
        assert_eq!(found.runtimes[0].version, "21.0.1");
        assert!(
            !serde_json::to_string(&found)
                .unwrap()
                .contains("never-return")
        );
    }

    #[test]
    fn replaces_only_vm_heap_flags_and_preserves_application_arguments() {
        let updated = memory_arguments(
            r#"-cp "C:\MC Libraries\*" -Xms1G -Xmx2G example.Main --name "" -XmxAPP"#,
            Some(6144),
        )
        .unwrap();
        assert_eq!(
            launch::split_windows_args(&updated).unwrap(),
            vec![
                "-cp",
                r"C:\MC Libraries\*",
                "-Xms3072M",
                "-Xmx6144M",
                "example.Main",
                "--name",
                "",
                "-XmxAPP"
            ]
        );
        let automatic = memory_arguments(&updated, None).unwrap();
        assert!(!automatic.contains("6144M"));
        assert!(automatic.contains("-XmxAPP"));
    }

    #[test]
    fn preserves_jar_module_targets_and_windows_escaping() {
        for input in [
            r#"-jar "C:\Game\client.jar" "a\"b" "C:\ends\\""#,
            "--module-path mods -m game/main",
            "--module=game/main",
        ] {
            let before = launch::split_windows_args(input).unwrap();
            let updated = memory_arguments(input, Some(4096)).unwrap();
            let mut after = launch::split_windows_args(&updated).unwrap();
            after.retain(|arg| !matches!(arg.as_str(), "-Xms2048M" | "-Xmx4096M"));
            assert_eq!(after, before);
        }
    }

    #[test]
    fn rejects_invalid_memory_hidden_flags_and_missing_launch_targets() {
        assert!(memory_arguments("Main", Some(511)).is_err());
        assert!(memory_arguments("Main", Some(65537)).is_err());
        assert!(memory_arguments("@secret-args.txt Main", Some(4096)).is_err());
        assert!(memory_arguments("-cp libs", Some(4096)).is_err());
        assert!(memory_arguments("-Xmx 4096M Main", Some(4096)).is_err());
        assert!(memory_arguments("-jar", Some(4096)).is_err());
        assert!(memory_arguments("--module=", Some(4096)).is_err());
        assert!(memory_arguments("-cp @args Main", Some(4096)).is_err());
    }

    #[test]
    fn external_launchers_cannot_receive_java_memory_flags() {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.minecraft_launcher = "curseforge".into();
        profile.game_exe_path = r"C:\Launchers\CurseForge.exe".into();
        assert!(
            prepare_java_arguments(profile, Some(4096))
                .unwrap_err()
                .contains("direct Java")
        );
    }
}
