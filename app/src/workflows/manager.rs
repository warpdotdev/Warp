use super::{workflow::Workflow, WorkflowObjectModel};
use crate::{
    cloud_object::{model::persistence::ObjectStoreModel, GenericStoredObject, Owner},
    drive::OpenWarpDriveObjectSettings,
    pane_group::{PaneContent, WorkflowPane},
    safe_warn,
    server::ids::{ClientId, SyncId},
    workflows::{workflow_view::WorkflowView, WorkflowViewMode},
    PaneViewLocator, WindowId,
};
use std::collections::{hash_map::Entry, HashMap};
use warpui::{Entity, EntityId, ModelContext, SingletonEntity};

pub struct WorkflowManager {
    panes_by_hashed_id: HashMap<String, WorkflowPaneData>,
}

#[derive(Debug, Clone)]
pub enum WorkflowOpenSource {
    Existing(SyncId),
    New {
        title: Option<String>,

        /// The "content" of the workflow.
        /// For `Command` workflows, this is the command.
        /// For `AgentMode` workflows, this is the AI query.
        content: Option<String>,

        owner: Owner,
        initial_folder_id: Option<SyncId>,
        is_for_agent_mode: bool,
    },
    NewFromWorkflow {
        workflow: Box<Workflow>,
        owner: Owner,
        initial_folder_id: Option<SyncId>,
    },
}

impl WorkflowManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        // OpenWarp:无云端 = 无 client_id→server_id 转换事件,原 UpdateManager
        // 订阅 + handle_update_manager_event 为死代码,Phase 2c‑1 删除。
        WorkflowManager {
            panes_by_hashed_id: HashMap::new(),
        }
    }

    pub fn find_pane(&self, source: &WorkflowOpenSource) -> Option<(WindowId, PaneViewLocator)> {
        match source {
            WorkflowOpenSource::Existing(workflow_id) => {
                let pane_data = self.panes_by_hashed_id.get(&workflow_id.uid())?;
                Some((pane_data.window_id, pane_data.locator))
            }
            WorkflowOpenSource::New { .. } | WorkflowOpenSource::NewFromWorkflow { .. } => None,
        }
    }

    pub fn create_pane(
        &mut self,
        source: &WorkflowOpenSource,
        settings: &OpenWarpDriveObjectSettings,
        mode: WorkflowViewMode,
        window_id: WindowId,
        ctx: &mut ModelContext<Self>,
    ) -> WorkflowPane {
        let view = ctx.add_typed_action_view(window_id, WorkflowView::new_in_pane);

        match source {
            WorkflowOpenSource::Existing(workflow_id) => {
                let workflow = ObjectStoreModel::as_ref(ctx)
                    .get_workflow(workflow_id)
                    .cloned();
                if let Some(workflow) = workflow {
                    view.update(ctx, |view, ctx| view.load(workflow, settings, mode, ctx));
                } else {
                    // If the workflow doesn't exist, try waiting for initial load and trying again
                    view.update(ctx, |view, ctx| {
                        view.wait_for_initial_load_then_load(
                            *workflow_id,
                            settings,
                            mode,
                            window_id,
                            ctx,
                        )
                    });
                }
            }
            WorkflowOpenSource::New {
                title,
                content,
                owner,
                initial_folder_id,
                is_for_agent_mode,
            } => view.update(ctx, |view, ctx| {
                view.open_new_workflow(
                    title.clone(),
                    content.clone(),
                    *owner,
                    *initial_folder_id,
                    *is_for_agent_mode,
                    SyncId::ClientId(ClientId::default()),
                    ctx,
                )
            }),
            WorkflowOpenSource::NewFromWorkflow {
                workflow,
                owner,
                initial_folder_id,
            } => {
                view.update(ctx, |view, ctx| {
                    view.load(
                        GenericStoredObject::new_local(
                            WorkflowObjectModel::new(*workflow.clone()),
                            *owner,
                            *initial_folder_id,
                            ClientId::default(),
                        ),
                        &OpenWarpDriveObjectSettings::default(),
                        mode,
                        ctx,
                    );
                });
            }
        }

        WorkflowPane::new(view, ctx)
    }

    pub fn register_pane(
        &mut self,
        pane: &WorkflowPane,
        pane_group_id: EntityId,
        window_id: WindowId,
        ctx: &mut ModelContext<Self>,
    ) {
        let workflow_id = pane.get_view(ctx).as_ref(ctx).workflow_id();
        let entry = self.panes_by_hashed_id.entry(workflow_id.uid());
        if let Entry::Vacant(entry) = entry {
            entry.insert(WorkflowPaneData {
                workflow_id,
                window_id,
                locator: PaneViewLocator {
                    pane_group_id,
                    pane_id: pane.id(),
                },
            });
        } else {
            safe_warn!(
                safe: ("Ignoring duplicate Workflow pane registration"),
                full: ("Ignoring duplicate Workflow pane registration for {workflow_id}")
            );
        }
    }

    pub fn deregister_pane(&mut self, pane: &WorkflowPane, ctx: &mut ModelContext<Self>) {
        let workflow_id = pane.get_view(ctx).as_ref(ctx).workflow_id();

        // If a workflow pane is restored, the workflow may have been reopened in the meantime. In
        // that case, don't let closing the original pane clear out the new pane.
        if let Entry::Occupied(entry) = self.panes_by_hashed_id.entry(workflow_id.uid()) {
            if entry.get().locator.pane_id == pane.id() {
                entry.remove();
            } else {
                log::warn!(
                    "Ignoring duplicate registration of panes for {}",
                    workflow_id.uid()
                );
            }
        }
    }

    pub fn reset(&mut self) {
        self.panes_by_hashed_id.clear();
    }
}

struct WorkflowPaneData {
    workflow_id: SyncId,
    window_id: WindowId,
    locator: PaneViewLocator,
}

impl Entity for WorkflowManager {
    type Event = ();
}

impl SingletonEntity for WorkflowManager {}
