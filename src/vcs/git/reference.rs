// This file is part of radicle-surf
// <https://github.com/radicle-dev/radicle-surf>
//
// Copyright (C) 2019-2020 The Radicle Team <dev@radicle.xyz>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License version 3 or
// later as published by the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::vcs::git::{
    object::{BranchName, Namespace, Oid, TagName},
    repo::RepositoryRef,
};
use regex::Regex;
use std::{fmt, str};
use thiserror::Error;

pub(super) mod glob;

/// TODO(finto): This isn't used currently, but it could be a better way of
/// passing around references.
/// A structured way of referring to a git reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ref {
    /// A git tag, which can be found under `.git/refs/tags/`.
    Tag {
        /// The name of the tag, e.g. `v1.0.0`.
        name: TagName,
    },
    /// A git branch, which can be found under `.git/refs/heads/`.
    LocalBranch {
        /// The name of the branch, e.g. `master`.
        name: BranchName,
    },
    /// A git branch, which can be found under `.git/refs/remotes/`.
    RemoteBranch {
        /// The remote name, e.g. `origin`.
        remote: String,
        /// The name of the branch, e.g. `master`.
        name: BranchName,
    },
    /// A git commit.
    Commit {
        /// The SHA value of the commit.
        sha: Oid,
    },
    /// A git namespace, which can be found under `.git/refs/namespaces/`.
    ///
    /// Note that namespaces can be nested.
    Namespace {
        /// The name value of the namespace.
        namespace: String,
        /// The reference under that namespace, e.g. The
        /// `refs/remotes/origin/master/ portion of `refs/namespaces/
        /// moi/refs/remotes/origin/master`.
        reference: Box<Ref>,
    },
}

impl Ref {
    /// We try and build a `Ref` based off of whether we have a list of
    /// namespaces or not.
    pub(crate) fn from_namespace_str(
        Namespace { values: namespaces }: &Namespace,
        spec: &str,
    ) -> Vec<Result<Ref, git2::Error>> {
        let maybe_commit = Oid::from_str(spec).map(|sha| Self::Commit { sha });

        let tag = Self::Tag {
            name: TagName::new(spec),
        };

        let local_branch = Self::LocalBranch {
            name: BranchName::new(spec),
        };

        let remote_branch = Self::RemoteBranch {
            remote: "**".to_string(),
            name: BranchName::new(spec),
        };

        if namespaces.is_empty() {
            vec![Ok(tag), Ok(local_branch), Ok(remote_branch), maybe_commit]
        } else {
            let mut ref_namespaces = vec![tag, local_branch, remote_branch];
            for namespace in namespaces.iter().rev() {
                for ref_namespace in ref_namespaces.iter_mut() {
                    *ref_namespace = Self::Namespace {
                        namespace: namespace.clone(),
                        reference: Box::new(ref_namespace.clone()),
                    };
                }
            }
            let mut ref_namespaces: Vec<Result<Ref, git2::Error>> =
                ref_namespaces.into_iter().map(Ok).collect();
            ref_namespaces.push(maybe_commit);

            ref_namespaces
        }
    }

    /// We try to find a [`git2::Commit`] based off of a `Ref` by either finding
    /// the commit by SHA, or turning the ref into a fully qualified ref
    /// (e.g. refs/remotes/**/master).
    pub(crate) fn find_ref<'a>(
        &self,
        repo: &RepositoryRef<'a>,
    ) -> Result<git2::Commit<'a>, git2::Error> {
        match self {
            Self::Commit { sha } => repo.repo_ref.find_commit(*sha),
            other => {
                let refglob = other.to_string();
                repo.repo_ref
                    .find_reference(&refglob)
                    .and_then(|reference| reference.peel_to_commit())
            },
        }
    }

    /// Given a list of `Ref`s, make a best effort to try find a
    /// [`git2::Commit`] by probing each `Ref` until we get a commit.
    /// Otherwise, we couldn't find it.
    pub(crate) fn try_find_commit<'a>(
        references: Vec<Result<Self, git2::Error>>,
        repo: &RepositoryRef<'a>,
    ) -> Option<git2::Commit<'a>> {
        for reference in references {
            match reference.and_then(|reference| reference.find_ref(repo)) {
                Ok(commit) => return Some(commit),
                Err(_) => continue,
            }
        }
        None
    }
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag { name } => write!(f, "refs/tags/{}", name),
            Self::LocalBranch { name } => write!(f, "refs/heads/{}", name),
            Self::RemoteBranch { remote, name } => write!(f, "refs/remotes/{}/{}", remote, name),
            Self::Namespace {
                namespace,
                reference,
            } => write!(f, "refs/namespaces/{}/{}", namespace, reference),
            Self::Commit { sha } => write!(f, "{}", sha.to_string()),
        }
    }
}

#[derive(Debug, PartialEq, Error)]
pub enum ParseError {
    #[error("was able to parse 'refs/remotes' but failed to parse the remote name, perhaps you're missing 'origin/'")]
    MissingRemote,
    #[error("was able to parse 'refs/namespaces' but failed to parse the namespace name, a valid form would be 'refs/namespaces/moi/refs/heads/master'")]
    MissingNamespace,
    #[error("the ref provided '{0}' was malformed")]
    MalformedRef(String),
    #[error("while attempting to parse a commit SHA we encountered an error: {0:?}")]
    Sha(#[from] git2::Error),
}

impl str::FromStr for Ref {
    type Err = ParseError;

    fn from_str(reference: &str) -> Result<Self, Self::Err> {
        lazy_static! {
            static ref REF: Regex = Regex::new(
                r"^(refs/remotes/|refs/tags/|refs/heads/|refs/namespaces/)([^refs/]\w+/)?(.*)"
            )
            .unwrap();
        }
        REF.captures(reference).map_or_else(
            || {
                Ok(Self::Commit {
                    sha: Oid::from_str(reference)?,
                })
            },
            |cap| {
                // Get the capture match for the prefix, i.e. 'refs/*'.
                // If we don't have a capture, we fall back to the reference string
                // in case it's a commit SHA.
                let prefix = cap
                    .get(1)
                    .ok_or_else(|| ParseError::MalformedRef(reference.to_string()))?
                    .as_str();

                // Get the capture match for the name, e.g. 'master'
                let name = cap
                    .get(3)
                    .ok_or_else(|| ParseError::MalformedRef(reference.to_string()))?
                    .as_str();

                // Matching on the prefix and falling back to commit if we don't find a match.
                match prefix {
                    "refs/remotes/" => match cap.get(2) {
                        None => Err(ParseError::MissingRemote),
                        Some(remote_name) => Ok(Self::RemoteBranch {
                            remote: remote_name.as_str().trim_end_matches('/').to_string(),
                            name: BranchName::new(name),
                        }),
                    },
                    "refs/heads/" => Ok(Self::LocalBranch {
                        name: BranchName::new(name),
                    }),
                    "refs/tags/" => Ok(Self::Tag {
                        name: TagName::new(name),
                    }),
                    "refs/namespaces/" => match cap.get(2) {
                        None => Err(ParseError::MissingNamespace),
                        Some(namespace) => Ok(Self::Namespace {
                            namespace: namespace.as_str().trim_end_matches('/').to_string(),
                            reference: Box::new(Ref::from_str(name)?),
                        }),
                    },
                    _ => Err(ParseError::MalformedRef(reference.to_string())),
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parse_ref() -> Result<(), ParseError> {
        assert_eq!(
            Ref::from_str("refs/remotes/origin/master"),
            Ok(Ref::RemoteBranch {
                remote: "origin".to_string(),
                name: BranchName::new("master")
            })
        );

        assert_eq!(
            Ref::from_str("refs/heads/master"),
            Ok(Ref::LocalBranch {
                name: BranchName::new("master"),
            })
        );

        assert_eq!(
            Ref::from_str("refs/tags/v0.0.1"),
            Ok(Ref::Tag {
                name: TagName::new("v0.0.1")
            })
        );

        assert_eq!(
            Ref::from_str("refs/namespaces/moi/refs/remotes/origin/master"),
            Ok(Ref::Namespace {
                namespace: "moi".to_string(),
                reference: Box::new(Ref::RemoteBranch {
                    remote: "origin".to_string(),
                    name: BranchName::new("master")
                })
            })
        );

        assert_eq!(
            Ref::from_str("refs/namespaces/moi/refs/namespaces/toi/refs/tags/v1.0.0"),
            Ok(Ref::Namespace {
                namespace: "moi".to_string(),
                reference: Box::new(Ref::Namespace {
                    namespace: "toi".to_string(),
                    reference: Box::new(Ref::Tag {
                        name: TagName::new("v1.0.0")
                    })
                })
            })
        );

        assert_eq!(
            Ref::from_str("3c78ee34dfc5fd4ae0cc7dd92d5a00a55197ba23"),
            Ok(Ref::Commit {
                sha: Oid::from_str("3c78ee34dfc5fd4ae0cc7dd92d5a00a55197ba23").unwrap()
            })
        );

        assert_eq!(
            Ref::from_str("refs/remotes/master"),
            Err(ParseError::MissingRemote),
        );

        assert_eq!(
            Ref::from_str("refs/namespaces/refs/remotes/origin/master"),
            Err(ParseError::MissingNamespace),
        );

        Ok(())
    }
}
