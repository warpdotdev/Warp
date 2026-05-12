use super::*;
use std::collections::HashSet;

#[test]
fn toggle_pin_in_set_flips_membership_for_each_call() {
    let mut pinned: HashSet<AIConversationId> = HashSet::new();
    let id = AIConversationId::new();

    // Initially empty.
    assert!(!pinned.contains(&id));

    // First toggle pins the id.
    toggle_pin_in_set(&mut pinned, id);
    assert!(pinned.contains(&id));
    assert_eq!(pinned.len(), 1);

    // Second toggle unpins it.
    toggle_pin_in_set(&mut pinned, id);
    assert!(!pinned.contains(&id));
    assert!(pinned.is_empty());

    // Third toggle re-pins, confirming the function is idempotent under
    // pairs and doesn't accumulate state.
    toggle_pin_in_set(&mut pinned, id);
    assert!(pinned.contains(&id));
}

#[test]
fn toggle_pin_in_set_only_affects_target_id() {
    let mut pinned: HashSet<AIConversationId> = HashSet::new();
    let a = AIConversationId::new();
    let b = AIConversationId::new();

    toggle_pin_in_set(&mut pinned, a);
    toggle_pin_in_set(&mut pinned, b);
    assert!(pinned.contains(&a));
    assert!(pinned.contains(&b));

    // Unpinning `a` leaves `b` alone.
    toggle_pin_in_set(&mut pinned, a);
    assert!(!pinned.contains(&a));
    assert!(pinned.contains(&b));
}
