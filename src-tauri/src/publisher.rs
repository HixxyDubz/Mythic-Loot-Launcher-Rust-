use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublisherStatus {
    pub gh_available: bool,
    pub authenticated: bool,
    pub account: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRequest {
    pub repository: String,
    pub description: String,
    pub visibility: RepositoryVisibility,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryVisibility {
    Private,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCreation {
    pub repository: String,
    pub url: String,
    pub message: String,
}

pub fn status() -> PublisherStatus {
    if run_gh(["--version"]).is_err() {
        return PublisherStatus {
            gh_available: false,
            authenticated: false,
            account: String::new(),
            message: "GitHub CLI is not installed or is not available on PATH".into(),
        };
    }
    let auth = match run_gh(["auth", "status", "--hostname", "github.com"]) {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return PublisherStatus {
                gh_available: true,
                authenticated: false,
                account: String::new(),
                message: output_message(&output, "GitHub CLI is not authenticated"),
            };
        }
        Err(error) => {
            return PublisherStatus {
                gh_available: true,
                authenticated: false,
                account: String::new(),
                message: error,
            };
        }
    };
    let account = run_gh(["api", "user", "--jq", ".login"])
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    PublisherStatus {
        gh_available: true,
        authenticated: true,
        message: if account.is_empty() {
            output_message(&auth, "GitHub CLI is authenticated")
        } else {
            format!("Authenticated as {account}")
        },
        account,
    }
}

pub fn create_repository(request: &RepositoryRequest) -> Result<RepositoryCreation, String> {
    validate_request(request)?;
    if !request.confirmed {
        return Err("Repository creation requires explicit confirmation".into());
    }
    let publisher = status();
    if !publisher.gh_available || !publisher.authenticated {
        return Err(publisher.message);
    }
    if repository_exists(&request.repository)? {
        return Err(format!(
            "GitHub repository {} already exists; select it instead of creating it",
            request.repository
        ));
    }

    let visibility = match request.visibility {
        RepositoryVisibility::Private => "--private",
        RepositoryVisibility::Public => "--public",
    };
    let output = run_gh([
        "repo",
        "create",
        request.repository.as_str(),
        visibility,
        "--description",
        request.description.trim(),
    ])?;
    if !output.status.success() {
        return Err(output_message(
            &output,
            "GitHub CLI could not create the repository",
        ));
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(RepositoryCreation {
        repository: request.repository.clone(),
        url,
        message: "Repository created. No modpack files have been uploaded yet.".into(),
    })
}

fn repository_exists(repository: &str) -> Result<bool, String> {
    let output = run_gh(["repo", "view", repository, "--json", "nameWithOwner"])?;
    Ok(output.status.success())
}

fn validate_request(request: &RepositoryRequest) -> Result<(), String> {
    validate_repository_name(&request.repository)?;
    if request.description.chars().count() > 350 {
        return Err("Repository description must be 350 characters or fewer".into());
    }
    Ok(())
}

pub fn validate_repository_name(value: &str) -> Result<(), String> {
    let mut parts = value.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return Err("Repository must use the owner/name format".into());
    }
    for (label, part) in [("owner", owner), ("name", name)] {
        if part.len() > 100
            || !part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            || part.starts_with('.')
            || part.ends_with('.')
            || part.contains("..")
        {
            return Err(format!(
                "Repository {label} contains unsupported characters"
            ));
        }
    }
    Ok(())
}

fn run_gh<'a>(arguments: impl IntoIterator<Item = &'a str>) -> Result<Output, String> {
    Command::new("gh")
        .args(arguments)
        .output()
        .map_err(|error| format!("Could not run GitHub CLI: {error}"))
}

fn output_message(output: &Output, fallback: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        fallback.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_github_owner_and_repository_names() {
        for value in ["HixxyDubz/Mythic-Loot-Modpack", "owner_2/pack.release-1"] {
            assert!(validate_repository_name(value).is_ok(), "rejected {value}");
        }
    }

    #[test]
    fn rejects_ambiguous_or_shell_shaped_repository_names() {
        for value in [
            "repo-only",
            "owner/repo/extra",
            "owner/../repo",
            "owner/repo name",
            "owner/repo;echo",
            "/repo",
        ] {
            assert!(validate_repository_name(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn repository_creation_is_fail_closed_without_confirmation() {
        let request = RepositoryRequest {
            repository: "owner/repo".into(),
            description: "fixture".into(),
            visibility: RepositoryVisibility::Private,
            confirmed: false,
        };
        assert!(
            create_repository(&request)
                .unwrap_err()
                .contains("explicit confirmation")
        );
    }
}
