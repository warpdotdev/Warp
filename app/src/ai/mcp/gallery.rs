use std::collections::HashMap;

use crate::ai::mcp::templatable::{GalleryData, JsonTemplate, TemplatableMCPServer};
use crate::server::datetime_ext::DateTimeExt;
use chrono::DateTime;
use uuid::Uuid;
use warpui::{Entity, ModelContext, SingletonEntity};

#[derive(Clone, Debug)]
pub struct GalleryMCPServer {
    uuid: Uuid,
    title: String,
    description: String,
    #[allow(dead_code)]
    version: i32,
    #[allow(dead_code)]
    instructions_in_markdown: Option<String>,
    json_template: JsonTemplate,
}

impl GalleryMCPServer {
    pub fn new(
        uuid: Uuid,
        title: String,
        description: String,
        version: i32,
        instructions_in_markdown: Option<String>,
        json_template: JsonTemplate,
    ) -> Self {
        Self {
            uuid,
            title,
            description,
            version,
            instructions_in_markdown,
            json_template,
        }
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn description(&self) -> String {
        self.description.clone()
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn json_template(&self) -> &JsonTemplate {
        &self.json_template
    }

    pub fn instructions_in_markdown(&self) -> Option<&String> {
        self.instructions_in_markdown.as_ref()
    }
}

impl TryFrom<GalleryMCPServer> for TemplatableMCPServer {
    type Error = String;

    fn try_from(gallery_server: GalleryMCPServer) -> Result<Self, Self::Error> {
        let GalleryMCPServer {
            uuid: gallery_uuid,
            title,
            description,
            version: gallery_version,
            instructions_in_markdown: _,
            json_template,
        } = gallery_server;

        Ok(TemplatableMCPServer {
            uuid: Uuid::new_v4(),
            name: title,
            description: Some(description),
            template: json_template,
            version: DateTime::now().timestamp(),
            gallery_data: Some(GalleryData {
                gallery_item_id: gallery_uuid,
                version: gallery_version,
            }),
        })
    }
}

pub struct MCPGalleryManager {
    gallery_items: HashMap<Uuid, GalleryMCPServer>,
    templatable_mcp_servers: HashMap<Uuid, TemplatableMCPServer>,
}

impl MCPGalleryManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        // OpenWarp(本地化,Phase 2d-2):原订阅 `UpdateManager` 的 `MCPGalleryUpdated` 事件
        // 于云端 fetch 后分发 gallery items。本地化后云端对象 fetch/fan-in 已删除，
        // 本 Phase 保持 gallery 为空并解除订阅 ——
        // gallery 在本地永远为空,由 `MCPServersListPageView` 渲染为空画布,本地 MCP 走 `file_based_manager`
        // 读 `~/.warp/mcp.json` 与 `~/.claude/...`,不受影响。
        Self {
            gallery_items: Default::default(),
            templatable_mcp_servers: Default::default(),
        }
    }

    pub fn get_gallery(&self) -> Vec<GalleryMCPServer> {
        self.gallery_items.values().cloned().collect()
    }

    pub fn get_gallery_item(&self, gallery_uuid: Uuid) -> Option<&GalleryMCPServer> {
        self.gallery_items.get(&gallery_uuid)
    }

    pub fn get_templatable_mcp_server(&self, gallery_uuid: Uuid) -> Option<&TemplatableMCPServer> {
        self.templatable_mcp_servers.get(&gallery_uuid)
    }
}

pub enum MCPGalleryManagerEvent {
    ItemsRefreshed,
}

impl Entity for MCPGalleryManager {
    type Event = MCPGalleryManagerEvent;
}

impl SingletonEntity for MCPGalleryManager {}
