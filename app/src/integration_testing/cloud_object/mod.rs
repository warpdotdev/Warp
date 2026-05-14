mod assertion;

pub use assertion::*;
use futures::{future::join_all, FutureExt};
use itertools::Itertools;
use std::future::Future;
use std::pin::Pin;
use warpui::{App, SingletonEntity};

use crate::{
    cloud_object::update_manager::UpdateManager,
    cloud_object::{model::persistence::ObjectStoreModel, Space},
};

/// Clears the object store of all non-welcome objects in the user's personal space.
/// Returns a future that resolves when the object store is cleared.
pub fn clear_object_store_model(app: &mut App) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let object_ids_to_delete =
        ObjectStoreModel::handle(app).read(app, |object_store_model, ctx| {
            object_store_model
                .active_non_welcome_cloud_objects_in_space(Space::Personal, ctx)
                .map(|object| object.object_type_and_id())
                .collect_vec()
        });

    let mut futures = Vec::new();
    for object_id in object_ids_to_delete {
        UpdateManager::handle(app).update(app, |update_manager, ctx| {
            update_manager.delete_object_by_user(object_id, ctx);
            if let Some(future_id) = update_manager.spawned_futures().last() {
                let future = ctx.await_spawned_future(*future_id);
                futures.push(future);
            }
        });
    }

    Box::pin(join_all(futures).map(|_| ()))
}
