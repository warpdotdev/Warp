use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub is_detached: bool,
    pub is_bare: bool,
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
    let mut out = Vec::new();
    let mut current: Option<PartialWorktree> = None;

    for line in input.lines() {
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
            },
            Worktree {
                path: PathBuf::from("/tmp/wt-b"),
                branch: None,
                head: None,
                is_detached: false,
                is_bare: false,
            },
        ];
        let current = current_worktree(&worktrees, Path::new("/tmp/wt-b"));
        assert_eq!(current.map(|wt| wt.name()), Some("wt-b".to_string()));
    }
}
