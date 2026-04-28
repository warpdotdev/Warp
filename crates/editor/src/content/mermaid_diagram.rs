use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use bytes::Bytes;
use mermaid_to_svg::MermaidTheme;
use warpui::{
    AppContext, SingletonEntity,
    assets::asset_cache::{AssetCache, AssetSource, AssetState, AsyncAssetId, AsyncAssetType},
    image_cache::ImageType,
    units::{IntoPixels, Pixels},
};

use crate::render::{
    layout::TextLayout,
    model::{BlockSpacing, ImageBlockConfig},
};

const DEFAULT_MERMAID_HEIGHT_LINE_MULTIPLIER: f32 = 10.0;

struct MermaidDiagramAsset;

impl AsyncAssetType for MermaidDiagramAsset {}

pub fn mermaid_asset_source(source: &str) -> AssetSource {
    let source = source.to_string();
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    let id = format!("light:{:x}", hasher.finish());
    let fetch_source = source.clone();

    AssetSource::Async {
        id: AsyncAssetId::new::<MermaidDiagramAsset>(id),
        fetch: Arc::new(move || {
            let source = fetch_source.clone();
            Box::pin(async move {
                mermaid_to_svg::render_mermaid_to_svg(&source, Some(&MermaidTheme::light()))
                    .map(|svg| Bytes::from(svg.into_bytes()))
                    .map_err(Into::into)
            })
        }),
    }
}

pub fn mermaid_diagram_layout(
    source: &str,
    layout: &TextLayout,
    spacing: BlockSpacing,
    app: &AppContext,
) -> (AssetSource, ImageBlockConfig) {
    let asset_source = mermaid_asset_source(source);
    let max_width = layout.max_width() - spacing.x_axis_offset();
    let default_height = layout.rich_text_styles().base_line_height()
        * DEFAULT_MERMAID_HEIGHT_LINE_MULTIPLIER.into_pixels();
    let (width, height) =
        mermaid_diagram_size(&asset_source, max_width, app).unwrap_or((max_width, default_height));

    (
        asset_source,
        ImageBlockConfig {
            width,
            height,
            spacing,
        },
    )
}

fn mermaid_diagram_size(
    asset_source: &AssetSource,
    max_width: Pixels,
    app: &AppContext,
) -> Option<(Pixels, Pixels)> {
    let asset_cache = AssetCache::as_ref(app);
    let AssetState::Loaded { data } = asset_cache.load_asset::<ImageType>(asset_source.clone())
    else {
        return None;
    };
    let ImageType::Svg { svg } = data.as_ref() else {
        return None;
    };
    let intrinsic_size = svg.size();
    let intrinsic_width = intrinsic_size.width();
    let intrinsic_height = intrinsic_size.height();
    if intrinsic_width <= 0. || intrinsic_height <= 0. {
        return None;
    }
    let width = Pixels::new(max_width.as_f32().min(intrinsic_width));
    let height = Pixels::new(width.as_f32() * intrinsic_height / intrinsic_width);
    Some((width, height))
}
