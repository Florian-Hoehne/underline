//! Wiki remotes for Underline.
//!
//! GitHub and GitLab both store a project's wiki as a separate git
//! repository of Markdown pages. This crate maps a project to that
//! repository so a workspace can be cloned, pulled and pushed with git.

use serde::{Deserialize, Serialize};

/// Where a wiki is hosted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    GitHub,
    GitLab,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum WikiError {
    #[error("`{0}` is not a GitHub or GitLab project URL")]
    UnsupportedUrl(String),
}

/// A project wiki that a workspace syncs with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiRemote {
    pub provider: Provider,
    /// Host, for example `github.com` or a self-hosted GitLab domain.
    pub host: String,
    /// Project path: `owner/repo` on GitHub, `group/subgroup/project` on GitLab.
    pub project: String,
}

impl WikiRemote {
    /// Parses a project web URL such as `https://github.com/owner/repo`.
    ///
    /// Hosts other than `github.com` are treated as GitLab, which covers
    /// gitlab.com and self-hosted instances.
    pub fn from_project_url(url: &str) -> Result<Self, WikiError> {
        let unsupported = || WikiError::UnsupportedUrl(url.to_owned());
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .ok_or_else(unsupported)?;
        let (host, path) = rest.split_once('/').ok_or_else(unsupported)?;
        let project = path
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .trim_end_matches(".wiki");
        let segments = project.split('/').filter(|s| !s.is_empty()).count();
        let provider = if host == "github.com" {
            Provider::GitHub
        } else {
            Provider::GitLab
        };
        let valid = match provider {
            Provider::GitHub => segments == 2,
            Provider::GitLab => segments >= 2,
        };
        if host.is_empty() || !valid {
            return Err(unsupported());
        }
        Ok(Self {
            provider,
            host: host.to_owned(),
            project: project.to_owned(),
        })
    }

    /// HTTPS clone URL of the wiki's git repository.
    pub fn clone_url(&self) -> String {
        format!("https://{}/{}.wiki.git", self.host, self.project)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_project_maps_to_wiki_repository() {
        let remote = WikiRemote::from_project_url("https://github.com/owner/repo/").unwrap();
        assert_eq!(remote.provider, Provider::GitHub);
        assert_eq!(remote.clone_url(), "https://github.com/owner/repo.wiki.git");
    }

    #[test]
    fn gitlab_nested_group_maps_to_wiki_repository() {
        let remote =
            WikiRemote::from_project_url("https://gitlab.example.com/group/sub/project.git")
                .unwrap();
        assert_eq!(remote.provider, Provider::GitLab);
        assert_eq!(
            remote.clone_url(),
            "https://gitlab.example.com/group/sub/project.wiki.git"
        );
    }

    #[test]
    fn rejects_urls_that_are_not_projects() {
        for url in [
            "github.com/owner/repo",
            "https://github.com/owner",
            "https://github.com/a/b/c",
        ] {
            assert!(WikiRemote::from_project_url(url).is_err(), "{url}");
        }
    }
}
