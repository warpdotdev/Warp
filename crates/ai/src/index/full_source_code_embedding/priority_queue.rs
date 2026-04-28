use std::hash::Hash;
use std::path::PathBuf;

use itertools::Itertools;
use priority_queue::PriorityQueue;

use crate::workspace::WorkspaceMetadata;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub(super) enum Priority {
    ActiveSession = 0,
    OpenSession = 1,
    PersistedSnapshot = 2,
}

#[derive(Debug, Clone)]
struct QueueEntry {
    metadata: WorkspaceMetadata,
}

impl Hash for QueueEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.metadata.path.hash(state);
    }
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.metadata.path == other.metadata.path
    }
}

impl Eq for QueueEntry {}

#[derive(Debug, Default)]
pub(super) struct BuildQueue {
    queue: PriorityQueue<QueueEntry, Priority>,
}

impl BuildQueue {
    pub(super) fn empty() -> Self {
        Self::default()
    }

    pub(super) fn queued_metadata(&self) -> impl IntoIterator<Item = WorkspaceMetadata> + use<'_> {
        self.queue.iter().map(|(entry, _)| entry.metadata.clone())
    }

    pub(super) fn new_with_persisted(snapshots_to_load: Vec<WorkspaceMetadata>) -> Self {
        let mut queue = PriorityQueue::new();
        queue.extend(
            snapshots_to_load
                .into_iter()
                .sorted_by(WorkspaceMetadata::most_recently_touched)
                .map(|entry| (QueueEntry { metadata: entry }, Priority::PersistedSnapshot)),
        );

        Self { queue }
    }

    /// Pulls the next index root path to sync from the priority queue and returns it.
    pub fn pick_next_sync(&mut self) -> Option<WorkspaceMetadata> {
        self.queue.pop().map(|(entry, _priority)| entry.metadata)
    }

    /// Adjusts the priority of a path in the queue if it exists.
    pub(super) fn update_path_priority(&mut self, root_path: PathBuf, priority: Priority) {
        // Exemplar is only used to lookup the item in the queue with the Eq implemented above
        // It will not overwrite the found item.
        let exemplar = QueueEntry {
            metadata: WorkspaceMetadata {
                path: root_path,
                ..Default::default()
            },
        };

        self.queue.change_priority(&exemplar, priority);
    }
}
