use crate::platform::WindowManager as _;
use crate::windowing::winit::window::WindowManager;
use crate::{DisplayId, DisplayIdx};
use anyhow::Result;
use itertools::Itertools as _;
use pathfinder_geometry::rect::RectF;
use std::sync::Arc;
use winit::monitor::MonitorHandle;
use winit::platform::windows::MonitorHandleExtWindows;
use winit::window::Window as WinitWindow;

use super::get_monitor_logical_bounds;

impl WindowManager {
    fn get_active_window_handle(&self) -> Result<Arc<WinitWindow>> {
        let window_id = &self
            .active_window_id()
            .ok_or(anyhow::anyhow!("No active window ID"))?;
        let ui_window = self
            .windows
            .get(window_id)
            .ok_or(anyhow::anyhow!("Window not found"))?;
        let winit_window_borrow = ui_window.inner.try_borrow()?;
        let winit_window_ref = winit_window_borrow
            .as_ref()
            .ok_or(anyhow::anyhow!("Unable to read Window information"))?;
        Ok(winit_window_ref.window.clone())
    }

    fn get_current_monitor_handle(&self) -> Result<MonitorHandle> {
        let winit_window_ref = self.get_active_window_handle()?;
        winit_window_ref
            .current_monitor()
            .ok_or(anyhow::anyhow!("Unable to get current monitor"))
    }

    pub(super) fn get_monitor_bounds_for_display_idx(&self, idx: DisplayIdx) -> Result<RectF> {
        let primary_monitor = self.get_primary_monitor_handle()?;
        let monitor = match idx {
            DisplayIdx::Primary => primary_monitor,
            DisplayIdx::External(numerical_index) => {
                let monitors = self.get_available_monitors()?;
                monitors
                    .iter()
                    .filter(|monitor| {
                        // Filter out the primary monitor.
                        monitor.hmonitor() != primary_monitor.hmonitor()
                    })
                    .nth(numerical_index)
                    .ok_or(anyhow::anyhow!(
                        "Could not find monitor handle for {numerical_index:?}"
                    ))?
                    .to_owned()
            }
        };
        Ok(get_monitor_logical_bounds(&monitor))
    }

    fn get_primary_monitor_handle(&self) -> Result<MonitorHandle> {
        let winit_window_ref = self.get_active_window_handle()?;
        winit_window_ref
            .primary_monitor()
            .ok_or(anyhow::anyhow!("No primary monitor found"))
    }

    pub(super) fn get_current_monitor_id(&self) -> Result<DisplayId> {
        let active_monitor = self.get_current_monitor_handle()?;
        let active_monitor_id = active_monitor.hmonitor();
        Ok(DisplayId::from(active_monitor_id as usize))
    }

    fn get_available_monitors(&self) -> Result<Vec<MonitorHandle>> {
        let winit_window_ref = self.get_active_window_handle()?;
        Ok(winit_window_ref.available_monitors().collect_vec())
    }

    pub(super) fn get_available_monitor_count(&self) -> Result<usize> {
        let winit_window_ref = self.get_active_window_handle()?;
        Ok(winit_window_ref.available_monitors().count())
    }

    pub(super) fn get_active_monitor_logical_bounds(&self) -> Result<RectF> {
        let active_monitor = self.get_current_monitor_handle()?;
        Ok(get_monitor_logical_bounds(&active_monitor))
    }
}
