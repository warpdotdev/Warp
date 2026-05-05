use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub is_detached: bool,
    pub is_bare: bool,
    /// Branch this worktree's branch was created from, parsed from the reflog
    /// "branch: Created from <ref>" entry. `None` if the reflog has been pruned
    /// or the branch was created in a way git doesn't record (e.g. renames).
    pub origin_branch: Option<String>,
}

impl Worktree {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }
}

pub fn parse_porcelain_list(input: &str) -> Vec<Worktree> {
    // Split on the optional `---ORIGIN---` marker. Everything before is standard
    // porcelain output; everything after is one `<path>|<branch>|<origin>` line per
    // worktree, produced by our extended shell command.
    const ORIGIN_MARKER: &str = "---ORIGIN---";
    let (porcelain_part, origin_part) = match input.find(ORIGIN_MARKER) {
        Some(idx) => (&input[..idx], Some(&input[idx + ORIGIN_MARKER.len()..])),
        None => (input, None),
    };

    let mut out = Vec::new();
    let mut current: Option<PartialWorktree> = None;

    for line in porcelain_part.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if let Some(wt) = current.take().and_then(PartialWorktree::finish) {
                out.push(wt);
            }
            continue;
        }

        let (key, value) = match line.split_once(' ') {
            Some(kv) => kv,
            None => (line, ""),
        };

        match key {
            "worktree" => {
                if let Some(wt) = current.take().and_then(PartialWorktree::finish) {
                    out.push(wt);
                }
                current = Some(PartialWorktree {
                    path: Some(PathBuf::from(value)),
                    branch: None,
                    head: None,
                    is_detached: false,
                    is_bare: false,
                });
            }
            "HEAD" => {
                if let Some(wt) = current.as_mut() {
                    wt.head = Some(value.to_string());
                }
            }
            "branch" => {
                if let Some(wt) = current.as_mut() {
                    wt.branch = Some(strip_refs_heads(value).to_string());
                }
            }
            "detached" => {
                if let Some(wt) = current.as_mut() {
                    wt.is_detached = true;
                }
            }
            "bare" => {
                if let Some(wt) = current.as_mut() {
                    wt.is_bare = true;
                }
            }
            _ => {}
        }
    }

    if let Some(wt) = current.and_then(PartialWorktree::finish) {
        out.push(wt);
    }

    // Apply origin info: each line in the origin section is `<path>|<branch>|<origin>`.
    // The origin field may be empty (no reflog entry found) — leave as None in that case.
    if let Some(origin_part) = origin_part {
        for line in origin_part.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, '|');
            let path = parts.next();
            let _branch = parts.next();
            let origin = parts.next();
            if let (Some(path), Some(origin)) = (path, origin) {
                if origin.is_empty() {
                    continue;
                }
                let path_buf = PathBuf::from(path);
                if let Some(wt) = out.iter_mut().find(|wt| wt.path == path_buf) {
                    wt.origin_branch = Some(origin.to_string());
                }
            }
        }
    }

    out
}

pub fn current_worktree<'a>(worktrees: &'a [Worktree], cwd: &Path) -> Option<&'a Worktree> {
    worktrees.iter().find(|wt| {
        let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let wt_canon = wt
            .path
            .canonicalize()
            .unwrap_or_else(|_| wt.path.clone());
        cwd_canon == wt_canon || cwd_canon.starts_with(&wt_canon)
    })
}

fn strip_refs_heads(s: &str) -> &str {
    s.strip_prefix("refs/heads/").unwrap_or(s)
}

struct PartialWorktree {
    path: Option<PathBuf>,
    branch: Option<String>,
    head: Option<String>,
    is_detached: bool,
    is_bare: bool,
}

impl PartialWorktree {
    fn finish(self) -> Option<Worktree> {
        let path = self.path?;
        Some(Worktree {
            path,
            branch: self.branch,
            head: self.head,
            is_detached: self.is_detached,
            is_bare: self.is_bare,
            origin_branch: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_worktree() {
        let input = "\
worktree /Users/me/proj
HEAD abc123
branch refs/heads/main
";
        let worktrees = parse_porcelain_list(input);
        assert_eq!(worktrees.len(), 1);
        let wt = &worktrees[0];
        assert_eq!(wt.path, PathBuf::from("/Users/me/proj"));
        assert_eq!(wt.head.as_deref(), Some("abc123"));
        assert_eq!(wt.branch.as_deref(), Some("main"));
        assert!(!wt.is_detached);
        assert!(!wt.is_bare);
    }

    #[test]
    fn parses_multiple_worktrees() {
        let input = "\
worktree /Users/me/proj
HEAD abc123
branch refs/heads/main

worktree /Users/me/proj-feature
HEAD def456
branch refs/heads/feature/login
";
        let worktrees = parse_porcelain_list(input);
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].name(), "proj");
        assert_eq!(worktrees[1].name(), "proj-feature");
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature/login"));
    }

    #[test]
    fn parses_detached_head_entry() {
        let input = "\
worktree /Users/me/proj-detached
HEAD abc123
detached
";
        let worktrees = parse_porcelain_list(input);
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_detached);
        assert!(worktrees[0].branch.is_none());
    }

    #[test]
    fn parses_bare_entry() {
        let input = "\
worktree /Users/me/proj.git
bare
";
        let worktrees = parse_porcelain_list(input);
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_bare);
        assert!(worktrees[0].head.is_none());
    }

    #[test]
    fn empty_input_yields_empty_vec() {
        assert!(parse_porcelain_list("").is_empty());
        assert!(parse_porcelain_list("   \n\n  ").is_empty());
    }

    #[test]
    fn name_falls_back_to_full_path_when_no_basename() {
        let wt = Worktree {
            path: PathBuf::from("/"),
            branch: None,
            head: None,
            is_detached: false,
            is_bare: false,
            origin_branch: None,
        };
        assert_eq!(wt.name(), "/");
    }

    #[test]
    fn current_worktree_matches_exact_path() {
        let worktrees = vec![
            Worktree {
                path: PathBuf::from("/tmp/wt-a"),
                branch: None,
                head: None,
                is_detached: false,
                is_bare: false,
                origin_branch: None,
            },
            Worktree {
                path: PathBuf::from("/tmp/wt-b"),
                branch: None,
                head: None,
                is_detached: false,
                is_bare: false,
                origin_branch: None,
            },
        ];
        let current = current_worktree(&worktrees, Path::new("/tmp/wt-b"));
        assert_eq!(current.map(|wt| wt.name()), Some("wt-b".to_string()));
    }

    #[test]
    fn parses_origin_section_into_matching_worktrees() {
        let input = "\
worktree /Users/me/proj
HEAD abc123
branch refs/heads/main

worktree /Users/me/proj-feat
HEAD def456
branch refs/heads/feature/auth
---ORIGIN---
/Users/me/proj|main|
/Users/me/proj-feat|feature/auth|main
";
        let worktrees = parse_porcelain_list(input);
        assert_eq!(worktrees.len(), 2);
        // Empty origin (root or unknown) leaves origin_branch as None.
        assert_eq!(worktrees[0].origin_branch, None);
        // Non-empty origin attaches to the matching path.
        assert_eq!(
            worktrees[1].origin_branch.as_deref(),
            Some("main")
        );
    }

    #[test]
    fn missing_origin_section_is_backward_compatible() {
        let input = "\
worktree /Users/me/proj
HEAD abc123
branch refs/heads/main
";
        let worktrees = parse_porcelain_list(input);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].origin_branch, None);
    }

    #[test]
    fn origin_section_ignores_unknown_paths() {
        let input = "\
worktree /Users/me/proj
HEAD abc123
branch refs/heads/main
---ORIGIN---
/Users/me/some-other-path|main|develop
";
        let worktrees = parse_porcelain_list(input);
        // Origin entry pointed at a path not in the porcelain list — silently dropped.
        assert_eq!(worktrees[0].origin_branch, None);
    }

    #[test]
    fn origin_section_ignores_malformed_lines() {
        let input = "\
worktree /Users/me/proj
HEAD abc123
branch refs/heads/main
---ORIGIN---
not enough fields
/Users/me/proj
/Users/me/proj|main|main
";
        let worktrees = parse_porcelain_list(input);
        // First two malformed lines skipped; third one is valid and applies.
        assert_eq!(worktrees[0].origin_branch.as_deref(), Some("main"));
    }
}
