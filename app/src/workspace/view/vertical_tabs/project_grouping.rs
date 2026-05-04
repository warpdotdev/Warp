use crate::code::view::CodeView;
use crate::pane_group::{PaneGroup, PaneId};
#[cfg(feature = "local_fs")]
use repo_metadata::repositories::DetectedRepositories;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use warpui::AppContext;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum ProjectGroupKey {
    Root(PathBuf),
    Cwd(PathBuf),
    // Carries a caller-scoped identity so unresolved tabs do not collapse together.
    Unknown(usize),
}

impl ProjectGroupKey {
    pub(super) fn label(&self) -> String {
        match self {
            ProjectGroupKey::Root(p) | ProjectGroupKey::Cwd(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
            ProjectGroupKey::Unknown(_) => "Other".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ProjectPaneGroup {
    pub(super) key: ProjectGroupKey,
    pub(super) member_indices: Vec<usize>,
}

pub(super) fn resolve_project_group_for_pane_group(
    pane_group: &PaneGroup,
    pane_ids: Option<&[PaneId]>,
    unknown_id: usize,
    app: &AppContext,
) -> ProjectGroupKey {
    let mut candidate_paths: Vec<PathBuf> = Vec::new();
    let mut push_candidate = |path: PathBuf| {
        if !candidate_paths.contains(&path) {
            candidate_paths.push(path);
        }
    };
    let pane_ids_to_scan =
        pane_ids.map_or_else(|| pane_group.visible_pane_ids(), <[PaneId]>::to_vec);
    for pane_id in pane_ids_to_scan {
        collect_pane_project_paths(pane_group, pane_id, app, &mut push_candidate);
    }

    #[cfg(feature = "local_fs")]
    {
        // Repo-root detection is local-FS only; other builds fall back to normalized paths.
        use warpui::SingletonEntity as _;
        let detected = DetectedRepositories::as_ref(app);
        if let Some(key) =
            repo_root_project_group_key(&candidate_paths, |path| detected.get_root_for_path(path))
        {
            return key;
        }
    }

    fallback_project_group_key(candidate_paths, unknown_id)
}

fn repo_root_project_group_key(
    candidate_paths: &[PathBuf],
    mut get_root_for_path: impl FnMut(&Path) -> Option<PathBuf>,
) -> Option<ProjectGroupKey> {
    for path in candidate_paths {
        if let Some(root) = get_root_for_path(path) {
            return Some(ProjectGroupKey::Root(normalize_project_group_path(root)));
        }
    }
    None
}

fn fallback_project_group_key(candidate_paths: Vec<PathBuf>, unknown_id: usize) -> ProjectGroupKey {
    if let Some(path) = candidate_paths.into_iter().next() {
        return ProjectGroupKey::Cwd(normalize_project_group_path(path));
    }
    ProjectGroupKey::Unknown(unknown_id)
}

fn collect_pane_project_paths(
    pane_group: &PaneGroup,
    pane_id: PaneId,
    app: &AppContext,
    push_candidate: &mut impl FnMut(PathBuf),
) {
    if let Some(view) = pane_group.terminal_view_from_pane_id(pane_id, app) {
        let view = view.as_ref(app);
        collect_terminal_project_paths(view, app, push_candidate);
    } else if let Some(code_pane) = pane_group.code_pane_by_id(pane_id) {
        let code_view = code_pane.file_view(app);
        let code_view = code_view.as_ref(app);
        collect_code_project_paths(code_view, app, push_candidate);
    }
}

fn collect_terminal_project_paths(
    view: &crate::terminal::TerminalView,
    app: &AppContext,
    push_candidate: &mut impl FnMut(PathBuf),
) {
    let is_local = view.active_session_is_local(app);
    // Unknown locality is not safe for raw path grouping; pwd_if_local still covers confirmed
    // local paths while remote sessions are initializing.
    if is_local == Some(false) {
        return;
    }
    if is_local == Some(true) {
        if let Some(repo_path) = view.current_repo_path() {
            push_candidate(repo_path.clone());
        }
    }
    if let Some(pwd) = view.pwd_if_local(app) {
        push_candidate(PathBuf::from(pwd));
    }
    if is_local == Some(true) {
        if let Some(pwd) = view.pwd() {
            push_candidate(PathBuf::from(pwd));
        }
    }
}

fn collect_code_project_paths(
    view: &CodeView,
    _app: &AppContext,
    push_candidate: &mut impl FnMut(PathBuf),
) {
    for path in view.local_paths() {
        push_candidate(
            path.parent()
                .map_or_else(|| path.clone(), |parent| parent.to_path_buf()),
        );
    }
}

fn normalize_project_group_path(path: PathBuf) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path;
    };
    if let Some(normalized) =
        normalize_wsl_drive_mount_path(path_str).or_else(|| normalize_windows_drive_path(path_str))
    {
        return PathBuf::from(normalized);
    }
    path
}

fn normalize_wsl_drive_mount_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/mnt/")?;
    let mut chars = rest.chars();
    let drive = chars.next()?;
    if !drive.is_ascii_alphabetic() || chars.next() != Some('/') {
        return None;
    }
    Some(format!(
        "{}:/{}",
        drive.to_ascii_lowercase(),
        chars.as_str()
    ))
}

fn normalize_windows_drive_path(path: &str) -> Option<String> {
    let mut chars = path.chars();
    let drive = chars.next()?;
    if !drive.is_ascii_alphabetic() || chars.next() != Some(':') {
        return None;
    }
    let rest = chars.as_str().replace('\\', "/");
    Some(format!("{}:{}", drive.to_ascii_lowercase(), rest))
}

pub(super) fn format_project_header_label(
    key: &ProjectGroupKey,
    tab_count: usize,
    pane_count: usize,
) -> String {
    let mut out = key.label();
    if tab_count > 1 {
        out.push_str(&format!("  ·  {tab_count} tabs"));
    }
    if pane_count > tab_count {
        out.push_str(&format!("  ·  {pane_count} panes"));
    }
    out
}

pub(super) fn group_by_project_key<F>(n: usize, key_fn: F) -> Vec<ProjectPaneGroup>
where
    F: Fn(usize) -> ProjectGroupKey,
{
    let mut order: Vec<ProjectGroupKey> = Vec::new();
    let mut buckets: HashMap<ProjectGroupKey, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let key = key_fn(i);
        if !buckets.contains_key(&key) {
            order.push(key.clone());
        }
        buckets.entry(key).or_default().push(i);
    }
    order
        .into_iter()
        .map(|k| {
            let v = buckets.remove(&k).unwrap_or_default();
            ProjectPaneGroup {
                key: k,
                member_indices: v,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(p: &str) -> ProjectGroupKey {
        ProjectGroupKey::Root(PathBuf::from(p))
    }
    fn cwd(p: &str) -> ProjectGroupKey {
        ProjectGroupKey::Cwd(PathBuf::from(p))
    }

    #[test]
    fn pre_resolved_repo_roots_group_together() {
        let keys = [
            root("/users/u/REPO/warp"),
            root("/users/u/REPO/warp"),
            root("/users/u/REPO/warp"),
        ];
        let groups = group_by_project_key(keys.len(), |i| keys[i].clone());
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, root("/users/u/REPO/warp"));
        assert_eq!(groups[0].member_indices, vec![0, 1, 2]);
    }

    #[test]
    fn unknown_falls_back_cleanly() {
        let keys = [
            root("/repo/warp"),
            ProjectGroupKey::Unknown(1),
            root("/repo/warp"),
        ];
        let groups = group_by_project_key(keys.len(), |i| keys[i].clone());
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].key, root("/repo/warp"));
        assert_eq!(groups[0].member_indices, vec![0, 2]);
        assert_eq!(groups[1].key, ProjectGroupKey::Unknown(1));
        assert_eq!(groups[1].member_indices, vec![1]);
    }

    #[test]
    fn unknown_tabs_do_not_group_together() {
        let keys = [
            ProjectGroupKey::Unknown(0),
            ProjectGroupKey::Unknown(1),
            ProjectGroupKey::Unknown(2),
        ];
        let groups = group_by_project_key(keys.len(), |i| keys[i].clone());
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].member_indices, vec![0]);
        assert_eq!(groups[1].member_indices, vec![1]);
        assert_eq!(groups[2].member_indices, vec![2]);
    }

    #[test]
    fn first_appearance_order_preserved() {
        let keys = [cwd("/a"), root("/b"), cwd("/a"), cwd("/c"), root("/b")];
        let groups = group_by_project_key(keys.len(), |i| keys[i].clone());
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].key, cwd("/a"));
        assert_eq!(groups[0].member_indices, vec![0, 2]);
        assert_eq!(groups[1].key, root("/b"));
        assert_eq!(groups[1].member_indices, vec![1, 4]);
        assert_eq!(groups[2].key, cwd("/c"));
        assert_eq!(groups[2].member_indices, vec![3]);
    }

    #[test]
    fn stable_order_within_group() {
        let keys = [
            root("/repo/warp"),
            cwd("/elsewhere"),
            root("/repo/warp"),
            root("/repo/warp"),
            cwd("/elsewhere"),
        ];
        let groups = group_by_project_key(keys.len(), |i| keys[i].clone());
        // Members within each group preserve their original ordering.
        assert_eq!(groups[0].key, root("/repo/warp"));
        assert_eq!(groups[0].member_indices, vec![0, 2, 3]);
        assert_eq!(groups[1].key, cwd("/elsewhere"));
        assert_eq!(groups[1].member_indices, vec![1, 4]);
    }

    #[test]
    fn repo_lookup_uses_raw_path_and_normalizes_returned_root() {
        let candidates = vec![PathBuf::from("/mnt/c/repo/foo")];
        let mut looked_up_paths = Vec::new();

        let key = repo_root_project_group_key(&candidates, |path| {
            looked_up_paths.push(path.to_path_buf());
            (path == Path::new("/mnt/c/repo/foo")).then_some(PathBuf::from("/mnt/c/repo/foo"))
        });

        assert_eq!(looked_up_paths, vec![PathBuf::from("/mnt/c/repo/foo")]);
        assert_eq!(key, Some(root("c:/repo/foo")));
    }

    #[test]
    fn wsl_and_windows_repo_roots_normalize_to_same_project_key() {
        let wsl_key = repo_root_project_group_key(&[PathBuf::from("/mnt/c/repo/foo")], |path| {
            (path == Path::new("/mnt/c/repo/foo")).then_some(PathBuf::from("/mnt/c/repo/foo"))
        })
        .expect("WSL path should resolve to a repo root");
        let windows_key = repo_root_project_group_key(&[PathBuf::from(r"C:\repo\foo")], |path| {
            (path == Path::new(r"C:\repo\foo")).then_some(PathBuf::from(r"C:\repo\foo"))
        })
        .expect("Windows path should resolve to a repo root");

        let keys = [wsl_key, windows_key];
        let groups = group_by_project_key(keys.len(), |i| keys[i].clone());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, root("c:/repo/foo"));
        assert_eq!(groups[0].member_indices, vec![0, 1]);
    }

    #[test]
    fn cwd_fallback_normalizes_project_key() {
        assert_eq!(
            fallback_project_group_key(vec![PathBuf::from("/mnt/c/repo/foo")], 7),
            cwd("c:/repo/foo")
        );
        assert_eq!(
            fallback_project_group_key(vec![PathBuf::from(r"C:\repo\foo")], 7),
            cwd("c:/repo/foo")
        );
    }

    #[test]
    fn label_uses_basename() {
        assert_eq!(root("/users/u/REPO/warp").label(), "warp");
        assert_eq!(root("/users/u/REPO/JOBBYJOB").label(), "JOBBYJOB");
        assert_eq!(cwd("/tmp/foo").label(), "foo");
    }

    #[test]
    fn unknown_label_is_other_and_no_detail() {
        assert_eq!(ProjectGroupKey::Unknown(0).label(), "Other");
    }

    #[test]
    fn header_label_single_tab_single_pane_is_compact() {
        // No counts when there is nothing extra to communicate.
        assert_eq!(format_project_header_label(&root("/r/warp"), 1, 1), "warp");
        assert_eq!(
            format_project_header_label(&ProjectGroupKey::Unknown(0), 1, 1),
            "Other"
        );
    }

    #[test]
    fn header_label_includes_tab_count_when_multi_tab() {
        assert_eq!(
            format_project_header_label(&root("/r/warp"), 2, 2),
            "warp  ·  2 tabs"
        );
        assert_eq!(
            format_project_header_label(&root("/r/warp"), 5, 5),
            "warp  ·  5 tabs"
        );
    }

    #[test]
    fn header_label_includes_pane_count_when_panes_exceed_tabs() {
        // Single multi-pane tab in the project: "warp · 2 panes".
        assert_eq!(
            format_project_header_label(&root("/r/warp"), 1, 2),
            "warp  ·  2 panes"
        );
        // Three tabs, one of which has two panes -> 4 panes total.
        assert_eq!(
            format_project_header_label(&root("/r/warp"), 3, 4),
            "warp  ·  3 tabs  ·  4 panes"
        );
    }

    #[test]
    fn header_label_omits_pane_count_when_equal_to_tab_count() {
        // 2 tabs / 2 panes -> redundant, so pane count is omitted.
        assert_eq!(
            format_project_header_label(&root("/r/warp"), 2, 2),
            "warp  ·  2 tabs"
        );
    }

    #[test]
    fn normalizes_wsl_drive_mount_paths() {
        assert_eq!(
            normalize_project_group_path(PathBuf::from("/mnt/c/repo/foo")),
            PathBuf::from("c:/repo/foo")
        );
        assert_eq!(
            normalize_project_group_path(PathBuf::from("/mnt/C/repo/foo")),
            PathBuf::from("c:/repo/foo")
        );
    }

    #[test]
    fn normalizes_windows_drive_paths() {
        assert_eq!(
            normalize_project_group_path(PathBuf::from(r"C:\repo\foo")),
            PathBuf::from("c:/repo/foo")
        );
    }

    #[test]
    fn leaves_non_drive_wsl_paths_alone() {
        assert_eq!(
            normalize_project_group_path(PathBuf::from("/home/user/repo")),
            PathBuf::from("/home/user/repo")
        );
    }
}
