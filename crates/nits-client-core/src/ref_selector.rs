//! Git-backed base/head selector state. The daemon supplies the catalog;
//! this module owns filtering and selection so every host behaves identically.

use nits_protocol::{BaseRefSpec, RefCandidate, RefSpec, RepoId, TargetRevision};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;

/// Which side of a review target the selector edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter)]
pub enum RefSelectorSide {
    Base,
    Head,
}

/// The asynchronous state shown inside the selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(RefSelectorStatusKind), derive(strum::EnumIter))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum RefSelectorStatus {
    Loading,
    Ready,
    Saving,
    InvalidRef { message: String },
    DaemonError { message: String },
}

/// One filtered selector row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefOption {
    pub ref_spec: RefSpec,
    pub subject: Option<String>,
    pub current: bool,
}

/// Everything a host needs to render the open selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefSelectorView {
    pub repo_id: RepoId,
    pub repo_name: String,
    pub side: RefSelectorSide,
    pub current: RefSpec,
    pub query: String,
    pub options: Vec<RefOption>,
    pub selected: usize,
    pub status: RefSelectorStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefSelector {
    catalog: Vec<RefCandidate>,
    pub(crate) view: RefSelectorView,
}

impl RefSelector {
    pub(crate) fn loading(
        repo_id: RepoId,
        repo_name: String,
        side: RefSelectorSide,
        current: RefSpec,
    ) -> Self {
        Self {
            catalog: Vec::new(),
            view: RefSelectorView {
                repo_id,
                repo_name,
                side,
                current,
                query: String::new(),
                options: Vec::new(),
                selected: 0,
                status: RefSelectorStatus::Loading,
            },
        }
    }

    pub(crate) fn install(&mut self, mut catalog: Vec<RefCandidate>) {
        if !catalog
            .iter()
            .any(|candidate| candidate.ref_spec == self.view.current)
        {
            catalog.insert(
                0,
                RefCandidate {
                    ref_spec: self.view.current.clone(),
                    subject: None,
                },
            );
        }
        self.catalog = catalog;
        self.view.status = RefSelectorStatus::Ready;
        self.filter();
    }

    pub(crate) fn query(&mut self, query: String) {
        self.view.query = query;
        self.filter();
    }

    pub(crate) fn step(&mut self, delta: i32) {
        let len = self.view.options.len();
        if len == 0 {
            self.view.selected = 0;
            return;
        }
        let current = self.view.selected.min(len - 1);
        let amount = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        self.view.selected = if delta.is_negative() {
            current.saturating_sub(amount)
        } else {
            current.saturating_add(amount).min(len - 1)
        };
    }

    pub(crate) fn revision_at(&self, index: usize) -> Option<TargetRevision> {
        let option = self.view.options.get(index)?;
        match self.view.side {
            RefSelectorSide::Head => Some(TargetRevision::Head {
                ref_spec: option.ref_spec.clone(),
            }),
            RefSelectorSide::Base => {
                base_ref(&option.ref_spec).map(|ref_spec| TargetRevision::Base { ref_spec })
            }
        }
    }

    fn filter(&mut self) {
        let query = self.view.query.trim().to_lowercase();
        self.view.options = self
            .catalog
            .iter()
            .filter(|candidate| match (&self.view.side, &candidate.ref_spec) {
                (RefSelectorSide::Base, RefSpec::WorkingTree) => false,
                (RefSelectorSide::Base | RefSelectorSide::Head, _) => true,
            })
            .filter(|candidate| {
                query.is_empty()
                    || fuzzy_match(&search_text(candidate).to_lowercase(), query.as_str())
            })
            .map(|candidate| RefOption {
                ref_spec: candidate.ref_spec.clone(),
                subject: candidate.subject.clone(),
                current: candidate.ref_spec == self.view.current,
            })
            .collect();
        self.view.selected = 0;
    }
}

fn base_ref(spec: &RefSpec) -> Option<BaseRefSpec> {
    match spec {
        RefSpec::Branch { name } => Some(BaseRefSpec::Branch { name: name.clone() }),
        RefSpec::Commit { oid } => Some(BaseRefSpec::Commit { oid: *oid }),
        RefSpec::Tag { name } => Some(BaseRefSpec::Tag { name: name.clone() }),
        RefSpec::Upstream => Some(BaseRefSpec::Upstream),
        RefSpec::Head => Some(BaseRefSpec::Head),
        RefSpec::WorkingTree => None,
    }
}

fn search_text(candidate: &RefCandidate) -> String {
    let (kind, value) = match &candidate.ref_spec {
        RefSpec::Branch { name } => ("branch", name.clone()),
        RefSpec::Commit { oid } => ("commit", oid.to_string()),
        RefSpec::Tag { name } => ("tag", name.clone()),
        RefSpec::WorkingTree => ("working tree", String::new()),
        RefSpec::Upstream => ("upstream", String::new()),
        RefSpec::Head => ("head", String::new()),
    };
    format!(
        "{kind} {value} {}",
        candidate.subject.as_deref().unwrap_or("")
    )
}

fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    let mut chars = needle.chars();
    let mut wanted = chars.next();
    for candidate in haystack.chars() {
        if wanted == Some(candidate) {
            wanted = chars.next();
        }
    }
    wanted.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_id() -> RepoId {
        RepoId::from_parts(1, 1)
    }

    #[test]
    fn fuzzy_match_is_an_ordered_subsequence() {
        assert!(fuzzy_match("branch feature/ref-selector", "frs"));
        assert!(!fuzzy_match("branch feature/ref-selector", "srf"));
    }

    #[test]
    fn working_tree_is_offered_only_for_head() {
        let catalog = vec![RefCandidate {
            ref_spec: RefSpec::WorkingTree,
            subject: None,
        }];
        let mut base = RefSelector::loading(
            repo_id(),
            "repo".into(),
            RefSelectorSide::Base,
            RefSpec::Head,
        );
        base.install(catalog.clone());
        assert!(
            base.view
                .options
                .iter()
                .all(|option| { !matches!(option.ref_spec, RefSpec::WorkingTree) })
        );

        let mut head = RefSelector::loading(
            repo_id(),
            "repo".into(),
            RefSelectorSide::Head,
            RefSpec::Head,
        );
        head.install(catalog);
        assert!(
            head.view
                .options
                .iter()
                .any(|option| { matches!(option.ref_spec, RefSpec::WorkingTree) })
        );
    }
}
