use crate::vcs::git::error::*;
use git2;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::str;

#[derive(Clone)]
pub struct Signature {
    pub name: String,
    pub email: String,
    pub time: git2::Time,
}

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Signature {{ name: {}, email: {} }}",
            self.name, self.email
        )
    }
}

impl<'repo> TryFrom<git2::Signature<'repo>> for Signature {
    type Error = str::Utf8Error;

    fn try_from(signature: git2::Signature) -> Result<Self, Self::Error> {
        let name = str::from_utf8(signature.name_bytes())?.into();
        let email = str::from_utf8(signature.email_bytes())?.into();
        let time = signature.when();

        Ok(Signature { name, email, time })
    }
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: git2::Oid,
    pub author: Signature,
    pub committer: Signature,
    pub message: String,
    pub summary: String,
}

impl<'repo> TryFrom<git2::Commit<'repo>> for Commit {
    type Error = Error;

    fn try_from(commit: git2::Commit) -> Result<Self, Self::Error> {
        let id = commit.id();
        let author = Signature::try_from(commit.author())?;
        let committer = Signature::try_from(commit.committer())?;
        let message_raw = commit.message_bytes();
        let message = str::from_utf8(message_raw)?.into();
        let summary_raw = commit.summary_bytes().expect("TODO");
        let summary = str::from_utf8(summary_raw)?.into();

        Ok(Commit {
            id,
            author,
            committer,
            message,
            summary,
        })
    }
}

/// A newtype wrapper over `String` to separate out
/// the fact that a caller wants to fetch a branch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BranchName(String);

impl TryFrom<&[u8]> for BranchName {
    type Error = str::Utf8Error;

    fn try_from(name: &[u8]) -> Result<Self, Self::Error> {
        str::from_utf8(name).map(|name| Self(String::from(name)))
    }
}

impl BranchName {
    pub fn new(name: &str) -> Self {
        BranchName(name.into())
    }

    pub fn name(&self) -> String {
        self.0.clone()
    }
}

/// The combination of a branch's name and where its locality (remote or local).
///
/// **Note**: The `PartialOrd` and `Ord` implementations compare on `BranchName`
/// only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: BranchName,
    pub locality: git2::BranchType,
}

impl PartialOrd for Branch {
    fn partial_cmp(&self, other: &Branch) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Branch {
    fn cmp(&self, other: &Branch) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Branch {
    /// Helper to create a remote `Branch` with a name
    pub fn remote(name: BranchName) -> Self {
        Branch {
            name,
            locality: git2::BranchType::Remote,
        }
    }

    /// Helper to create a remote `Branch` with a name
    pub fn local(name: BranchName) -> Self {
        Branch {
            name,
            locality: git2::BranchType::Local,
        }
    }

    fn from_reference(reference: &git2::Reference) -> Option<Result<Branch, str::Utf8Error>> {
        if !reference.is_branch() {
            return None;
        }

        let name = BranchName::try_from(reference.name_bytes());
        let locality = if reference.is_remote() {
            git2::BranchType::Remote
        } else {
            git2::BranchType::Local
        };
        Some(name.map(|name| Branch { name, locality }))
    }
}

/// A newtype wrapper over `String` to separate out
/// the fact that a caller wants to fetch a tag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TagName(String);

impl TryFrom<&[u8]> for TagName {
    type Error = str::Utf8Error;

    fn try_from(name: &[u8]) -> Result<Self, Self::Error> {
        str::from_utf8(name).map(|name| Self(String::from(name)))
    }
}

impl TagName {
    pub fn new(name: &str) -> Self {
        TagName(name.into())
    }

    pub fn name(&self) -> String {
        self.0.clone()
    }
}

pub struct Tag {
    pub id: git2::Oid,
    pub name: TagName,
    pub tagger: Option<Signature>,
    pub message: Option<String>,
}

impl<'repo> TryFrom<git2::Tag<'repo>> for Tag {
    type Error = str::Utf8Error;

    fn try_from(tag: git2::Tag) -> Result<Self, Self::Error> {
        let id = tag.id();

        let name = TagName::try_from(tag.name_bytes())?;

        let tagger = tag.tagger().map(Signature::try_from).transpose()?;

        let message = tag
            .message_bytes()
            .map(str::from_utf8)
            .transpose()?
            .map(|message| message.into());

        Ok(Tag {
            id,
            name,
            tagger,
            message,
        })
    }
}

/// A newtype wrapper over `String` to separate out
/// the fact that a caller wants to fetch a commit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sha1(git2::Oid);

impl Sha1 {
    pub fn value(&self) -> &git2::Oid {
        &self.0
    }
}

/// An enumeration of git objects we can fetch and turn
/// into a [`History`](struct.History.html).
#[derive(Debug, Clone)]
pub enum Object {
    Branch(BranchName),
    Tag(TagName),
}

impl Object {
    pub fn branch(name: &str) -> Self {
        Object::Branch(BranchName::new(name))
    }

    pub fn tag(name: &str) -> Self {
        Object::Tag(TagName::new(name))
    }

    pub fn name(&self) -> String {
        match self {
            Object::Branch(name) => name.0.clone(),
            Object::Tag(name) => name.0.clone(),
        }
    }
}

pub enum RevObject {
    Branch(Branch),
    Tag(Tag),
    Commit(Commit),
}

impl RevObject {
    pub fn from_revparse(repo: git2::Repository, spec: &str) -> Result<Self, Error> {
        let (object, optional_ref) = repo.revparse_ext(spec)?;

        match optional_ref {
            Some(reference) => match Branch::from_reference(&reference) {
                Some(branch_result) => Ok(RevObject::Branch(branch_result?)),
                None => Err(Error::RevParseFailure),
            },
            None => {
                let tag = object.into_tag().map(Tag::try_from);
                match tag {
                    Ok(tag) => Ok(RevObject::Tag(tag?)),
                    Err(object) => {
                        let commit = object.into_commit().map(Commit::try_from);
                        match commit {
                            Ok(commit) => Ok(RevObject::Commit(commit?)),
                            Err(_object) => Err(Error::RevParseFailure),
                        }
                    }
                }
            }
        }
    }
}
