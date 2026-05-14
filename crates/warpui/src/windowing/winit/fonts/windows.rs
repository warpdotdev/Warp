use super::{
    font_handle::FontHandle, FontFamily, LoadedSystemFonts, TextLayoutSystem,
    ValidateFontSupportsEn,
};
use crate::fonts::FontId;
use anyhow::Result;
use font_kit::loader::Loader as _;
use font_kit::{
    family_name::FamilyName as FKFamilyName, properties::Properties as FKProperties,
    properties::Style as FKStyle, properties::Weight as FKWeight, source::SystemSource as FKSource,
};
use itertools::Itertools;
use owned_ttf_parser::OwnedFace;
use std::collections::HashMap;
use std::sync::Arc;

/// 返回用于偏置 DirectWrite Han 字形回退的 BCP-47 locale 字符串。
/// 与当前 UI locale 同步(由 `app::i18n` 通过 `crate::set_ui_locale` 设置)。
fn current_fallback_locale() -> String {
    crate::current_ui_locale()
}

/// Windows symbol fonts that are used to render window control icons. We specifically do not do any
/// validation of these fonts (i.e. to check if the font contains english characters).
const SYMBOL_ICON_FONTS: &[&str] = &["Segoe Fluent Icons", "Segoe MDL2 Assets"];

pub(crate) mod loader {
    use crate::fonts::FontInfo;

    use super::*;

    pub fn load_all_system_fonts() -> LoadedSystemFonts {
        let source = font_kit::source::SystemSource::new();
        let fonts = match source.all_fonts() {
            Ok(fonts) => fonts,
            Err(err) => {
                log::warn!("unable to retrieve all fonts from DirectWrite source: {err:?}");
                return LoadedSystemFonts(vec![]);
            }
        };

        let mut family_map = HashMap::new();

        for font_handle in fonts.into_iter() {
            if let Ok(font) = font_handle.load() {
                let family_name = font.family_name();
                let is_monospace = font.is_monospace();

                if font.glyph_for_char('m').is_none() {
                    // Only allow the user to select fonts that have an English character set.
                    log::debug!("skipping family {family_name:?} because no 'm' glyph was found");
                    continue;
                }
                // Convert font_kit::Handle into UI framework-specific FontHandle.
                let font_handle = match font_handle {
                    font_kit::handle::Handle::Path { path, font_index } => {
                        FontHandle::new(path, font_index, is_monospace)
                    }
                    font_kit::handle::Handle::Memory { bytes, font_index } => {
                        let owned_face_result = match Arc::try_unwrap(bytes) {
                            // If we can ensure ownership of the bytes, create an OwnedFace without copying.
                            Ok(owned_bytes) => OwnedFace::from_vec(owned_bytes, font_index),
                            // If we can't get sole ownership, create on OwnedFace from a copy the bytes
                            // (created by .to_vec()).
                            Err(shared_bytes) => {
                                OwnedFace::from_vec(shared_bytes.to_vec(), font_index)
                            }
                        };
                        match owned_face_result {
                            Ok(typeface) => FontHandle::from(typeface),
                            Err(err) => {
                                // If we can't parse the typeface, skip it.
                                log::warn!(
                                    "unable to parse typeface from family {family_name}: {err:?}"
                                );
                                continue;
                            }
                        }
                    }
                };

                let (entry_info, entry_family) = family_map
                    .entry(family_name.clone())
                    .or_insert_with(move || {
                        (
                            FontInfo {
                                family_name: family_name.clone(),
                                is_monospace,
                            },
                            FontFamily {
                                name: family_name,
                                fonts: vec![],
                            },
                        )
                    });
                entry_info.is_monospace |= is_monospace;
                entry_family.fonts.push(font_handle);
            }
        }
        LoadedSystemFonts(family_map.into_values().collect_vec())
    }

    pub fn load_system_font(font_family: &str) -> Result<FontFamily> {
        let source = font_kit::source::SystemSource::new();
        let family = source.select_family_by_name(font_family)?;

        let validate_supports_en = if SYMBOL_ICON_FONTS.contains(&font_family) {
            ValidateFontSupportsEn::No
        } else {
            ValidateFontSupportsEn::Yes
        };

        Ok(FontFamily {
            name: font_family.to_string(),
            fonts: family
                .fonts()
                .iter()
                .flat_map(|font_kit_handle| {
                    load_font_from_handle(font_kit_handle, validate_supports_en)
                })
                .collect_vec(),
        })
    }
}

impl TextLayoutSystem {
    /// Given a specific character and FontID, find alternate system fonts that can
    /// render that character.
    pub fn get_fallback_fonts_for_character(
        &self,
        character: char,
        font_id: FontId,
    ) -> Result<Vec<FontId>> {
        // Retrieve the font's family name and properties from the font store.
        // First, find the font's fontdb ID.
        let &original_font_id =
            self.font_id_map
                .read()
                .get_by_left(&font_id)
                .ok_or(anyhow::format_err!(
                    "No left entry found for {font_id:?} in font_id_map"
                ))?;
        let (style, weight, family_name) = self.get_font_info_from_store(original_font_id)?;
        let source = FKSource::new();
        let style = match style {
            fontdb::Style::Normal => FKStyle::Normal,
            fontdb::Style::Italic => FKStyle::Italic,
            fontdb::Style::Oblique => FKStyle::Oblique,
        };
        let weight = FKWeight(weight.0 as f32);
        let properties = FKProperties {
            style,
            weight,
            stretch: Default::default(),
        };

        let font_handle = source
            .select_best_match(
                &[
                    FKFamilyName::Title(family_name.to_owned()),
                    FKFamilyName::Monospace,
                ],
                &properties,
            )
            .map_err(|err| anyhow::anyhow!("Didn't find {family_name} in fontdb: {err}"))?;

        // Load fallback fonts for the requested character.
        let loaded_font = font_handle.load().map_err(|err| {
            anyhow::anyhow!("Unable to load typeface from font_kit Handle: {err:?}")
        })?;

        let locale = current_fallback_locale();

        // 按 locale 优先的 CJK 系统字体:DirectWrite 的 IDWriteFontFallback 在 Windows 英文 / 开发环境
        // 下不参考 locale 解决 Han 字形歧义,默认返回 Microsoft YaHei,导致日文 UI 反倒拿到简体字字形。
        // 因此对共享 CJK Han 字符,我们在 DirectWrite 回退之前先 prepend 当前 locale 对应的系统字体
        // (例如 ja-* → Yu Gothic UI)。
        let mut fallback_font_vec: Vec<FontId> = Vec::new();
        if crate::is_shared_cjk_han(character) {
            for family in preferred_cjk_families_for_locale(&locale) {
                if let Ok(fam) = source.select_family_by_name(family) {
                    for fk_handle in fam.fonts() {
                        if let Ok(handle) =
                            load_font_from_handle(fk_handle, ValidateFontSupportsEn::No)
                        {
                            if let Ok(id) = self.insert_font(handle) {
                                fallback_font_vec.push(id);
                            }
                        }
                    }
                    if !fallback_font_vec.is_empty() {
                        break;
                    }
                }
            }
        }

        let fallback_result = loaded_font.get_fallbacks(character.to_string().as_str(), &locale);

        // Convert each font-kit fallback `Font` into a UI framework `FontHandle` and load it into
        // fontdb. We deliberately avoid `font_kit::Font::handle()` here: its default impl reads
        // the full font file into an `Arc<Vec<u8>>` and returns a `Handle::Memory` with
        // `font_index` hard-coded to `0` (see the FIXME at font-kit/src/loader.rs:172), which
        // bypasses `TextLayoutSystem::insert_font`'s path-based dedup and loses TTC face indices.
        // Instead we reach through `NativeFont` to the underlying `IDWriteFontFace` and recover
        // the on-disk file path + real face index, the same way
        // `DirectWriteSource::create_handle_from_dwrite_font` does for enumerated system fonts.
        // This lets fontdb mmap the file lazily and lets `insert_font` dedup by `(path, index)`,
        // so the same fallback family is loaded at most once per process.
        fallback_font_vec.extend(fallback_result.fonts.into_iter().flat_map(|fallback_font| {
            let loaded_handle = fallback_font_path_handle(&fallback_font.font).or_else(|| {
                // Last-resort fallback for fonts that aren't backed by a local file (e.g.
                // custom collection loaders). These don't appear in practice for DirectWrite
                // system fallbacks, but preserve the original byte-copy behavior so we
                // degrade gracefully instead of dropping the glyph.
                let handle = fallback_font.font.handle()?;
                load_font_from_handle(&handle, ValidateFontSupportsEn::No).ok()
            })?;
            self.insert_font(loaded_handle).ok()
        }));

        Ok(fallback_font_vec)
    }

    /// 启动期预热当前 UI locale 偏好的 CJK 字体族(`preferred_cjk_families_for_locale`),
    /// 在 `FontDB` 构造后立即同步调用一次。
    ///
    /// 修复 zerx-lab/warp#68 「启动后中文字体渲染出错,关闭面板重开才好」的回归:
    /// PR #62 在 `get_fallback_fonts_for_character` 中按 locale prepend 系统 CJK 字体到
    /// cosmic-text 的回退链;但首屏首次触发 CJK 回退时,`SystemSource::select_family_by_name`
    /// 在 Windows DirectWrite cold path 下偶发拿不到字体,prepend 段为空,回退落到
    /// `IDWriteFontFallback::MapCharacters` 的 cold 输出(可能给出非 locale 偏好的家族)。
    /// 一旦该结果写入 cosmic-text 的 `font_codepoint_support_info_cache` /
    /// `shape_run_cache`(FontSystem 实例级、locale 不变不会失效),后续渲染会一直复用错误回退,
    /// 直到面板销毁/字号/font_id 变化绕过 cache key 才会重走一次。
    ///
    /// 预热在这里同步把 preferred 家族灌进 fontdb(`insert_font` 走
    /// `loaded_fonts` 按 `(path, index)` 去重,后续 `get_fallback_fonts_for_character` 命中时
    /// 直接返回已存在的 `FontId`,不会重复加载),消除 cold path 的不确定性。
    ///
    /// 性能开销:启动期一次性构造 `SystemSource`,并 select、load、insert 一个
    /// preferred family。`load_font_from_handle` 走 font_kit Path 句柄转 `OwnedFace`,
    /// fontdb 内部 mmap lazily。在 Windows 11 + 装机自带 YaHei UI 上实测不过数毫秒,
    /// 且净收益为正 —— 之前 `get_fallback_fonts_for_character` 每次 CJK 字符 cache miss
    /// 都会新建一次 `SystemSource` 并重新 select/load,预热后这条路径首屏即命中已加载 FontId。
    ///
    /// 非 CJK locale 也会预热 Windows 默认简中 UI 字体族,保证英文 UI 下的中文文件名
    /// 等普通 `Text` 元素首帧就有可用 Han 字形,且不需要枚举全部系统字体。
    ///
    /// 失败(系统未装该 family / 句柄加载失败)只记 warn,不影响启动 —— 此时退化到
    /// DirectWrite 默认回退。
    pub(crate) fn warm_up_preferred_cjk_families(&self) {
        let locale = current_fallback_locale();
        let families = preferred_cjk_families_for_locale(&locale);
        if families.is_empty() {
            return;
        }
        let source = FKSource::new();
        let mut warmed_any = false;
        for family in families {
            let Ok(fam) = source.select_family_by_name(family) else {
                // 系统未装该 family(例如纯净版 Windows 11 可能没有 SimSun) —— 继续试下一个。
                continue;
            };
            let mut family_loaded = false;
            for fk_handle in fam.fonts() {
                match load_font_from_handle(fk_handle, ValidateFontSupportsEn::No) {
                    Ok(handle) => {
                        if self.insert_font(handle).is_ok() {
                            family_loaded = true;
                        }
                    }
                    Err(err) => {
                        log::debug!(
                            "warm_up_preferred_cjk_families: 跳过 {family:?} 的一个 face: {err:?}"
                        );
                    }
                }
            }
            if family_loaded {
                warmed_any = true;
                // 与 `get_fallback_fonts_for_character` 的「一个 family 命中即 break」行为对齐,
                // 避免预热超过实际回退会使用的字体集合。
                break;
            }
        }
        if !warmed_any {
            log::warn!(
                "warm_up_preferred_cjk_families: locale={locale:?} 下未能预热任何 CJK family ({families:?}) —— 首屏 CJK 回退将走 DirectWrite cold path"
            );
        }
    }

    /// Critical section for fetching the font style, weight and family name from fontdb.
    /// This function performs the minimum work required to fetch this information from
    /// fontdb to minimize the amount of time spent holding a read lock on the font store.
    fn get_font_info_from_store(
        &self,
        font_id: fontdb::ID,
    ) -> Result<(fontdb::Style, fontdb::Weight, String)> {
        let store_read_lock = self.font_store.read();
        let db_read = store_read_lock.db();
        let face = db_read.face(font_id).ok_or(anyhow::anyhow!(
            "Unable to retrieve font face from fontdb font_store"
        ))?;
        let style = face.style;
        let weight = face.weight;
        let Some(en_us_family_info) = face.families.first() else {
            return Err(anyhow::anyhow!("Font face doesn't have any family names"));
        };
        let (family_name, _) = en_us_family_info;
        // Clone the family name because it's protected by the font store's RWLock.
        Ok((style, weight, family_name.to_owned()))
    }
}

fn load_font_from_handle(
    font_handle: &font_kit::handle::Handle,
    validate_supports_en_charset: ValidateFontSupportsEn,
) -> Result<FontHandle> {
    let font = font_handle.load()?;
    let is_monospace = font.is_monospace();
    if matches!(validate_supports_en_charset, ValidateFontSupportsEn::Yes) {
        font.glyph_for_char('m').ok_or(anyhow::format_err!(
            "No 'm' glyph found for font {}",
            font.full_name()
        ))?;
    }
    match font_handle {
        font_kit::handle::Handle::Path { path, font_index } => {
            Ok(FontHandle::new(path, *font_index, is_monospace))
        }
        font_kit::handle::Handle::Memory { bytes, font_index } => {
            let typeface = OwnedFace::from_vec(bytes.to_vec(), *font_index)?;
            Ok(FontHandle::from(typeface))
        }
    }
}

/// 提取 BCP-47 标签的主语言子标签(primary subtag),已统一 ASCII 小写。
/// 例如 `ja-jp` → `ja`、`zh-hant-tw` → `zh`、`kok-in` → `kok`。
/// 用于精确判断主语言,避免 `starts_with("ko")` 这类前缀匹配把
/// `kok-IN`(孔卡尼语)误判为韩文,或 `zha-CN`(壮语)误判为中文。
fn primary_subtag(lower: &str) -> &str {
    lower.split(['-', '_']).next().unwrap_or("")
}

const SIMPLIFIED_CHINESE_CJK_FAMILIES: &[&str] =
    &["Microsoft YaHei UI", "Microsoft YaHei", "SimSun"];
const TRADITIONAL_CHINESE_CJK_FAMILIES: &[&str] = &[
    "Microsoft JhengHei UI",
    "Microsoft JhengHei",
    "PMingLiU",
    "MingLiU",
];
const JAPANESE_CJK_FAMILIES: &[&str] = &[
    "Yu Gothic UI",
    "Yu Gothic",
    "Meiryo UI",
    "Meiryo",
    "MS Gothic",
];
const KOREAN_CJK_FAMILIES: &[&str] = &["Malgun Gothic", "Gulim", "Dotum"];

/// 按 locale 优先返回 Windows 系统 CJK 字体族(按优先级)。
/// 用于覆盖 DirectWrite 不参考 locale 的 Han 回退。
///
/// 路由同时识别 BCP-47 region 子标签(zh-TW / zh-HK / zh-MO)和 script 子标签
/// (zh-Hant / zh-Hans,可带 region:zh-Hant-TW 等),调用方无需事先规范化 tag。
/// 非 CJK locale 使用简中字体族作为稳定兜底,避免英文 UI 下中文文件名首帧缺字。
fn preferred_cjk_families_for_locale(locale: &str) -> &'static [&'static str] {
    let lower = locale.to_ascii_lowercase();
    match primary_subtag(&lower) {
        "ja" => JAPANESE_CJK_FAMILIES,
        "ko" => KOREAN_CJK_FAMILIES,
        "zh" if is_zh_traditional(&lower) => TRADITIONAL_CHINESE_CJK_FAMILIES,
        "zh" => SIMPLIFIED_CHINESE_CJK_FAMILIES,
        _ => SIMPLIFIED_CHINESE_CJK_FAMILIES,
    }
}

/// `lower`(已 ASCII 小写化的 BCP-47 标签)是否指向繁体中文。
/// 同时匹配 region 形式(zh-tw / zh-hk / zh-mo)和 script 子标签形式
/// (zh-hant、zh-hant-tw、zh-foo-hant 等)。要求连字符边界,
/// 避免 `zh-hansolo` 之类意外匹配。
fn is_zh_traditional(lower: &str) -> bool {
    if primary_subtag(lower) != "zh" {
        return false;
    }
    if lower.starts_with("zh-tw") || lower.starts_with("zh-hk") || lower.starts_with("zh-mo") {
        return true;
    }
    // 遍历主标签后的连字符子标签。
    lower.split('-').skip(1).any(|sub| sub == "hant")
}

/// Builds a path-backed [`FontHandle`] for a font-kit DirectWrite `Font` by reaching through
/// [`font_kit::loaders::directwrite::NativeFont`] to the underlying `IDWriteFontFace`.
///
/// This mirrors what font-kit itself does for enumerated system fonts in
/// `DirectWriteSource::create_handle_from_dwrite_font` (font-kit/src/sources/directwrite.rs:103),
/// and is the reason we carry `dwrote` as a direct dependency: font-kit's generic
/// `Loader::handle()` default returns a `Handle::Memory` with a byte copy of the full file, which
/// we specifically need to avoid on the per-character fallback path.
///
/// Returns `None` when DirectWrite cannot produce a local file path for the font, i.e. the font
/// was loaded via a custom collection loader or backed only by an in-memory stream. For system
/// fallback fonts returned by `IDWriteFontFallback::MapCharacters` against the system font
/// collection, a path is always available.
fn fallback_font_path_handle(font: &font_kit::loaders::directwrite::Font) -> Option<FontHandle> {
    let native = font.native_font();
    let file = native.dwrite_font_face.files().ok()?.into_iter().next()?;
    let path = file.font_file_path().ok()?;
    let font_index = native.dwrite_font_face.get_index();
    Some(FontHandle::new(path, font_index, font.is_monospace()))
}

#[cfg(test)]
mod tests {
    use super::{
        preferred_cjk_families_for_locale, JAPANESE_CJK_FAMILIES, KOREAN_CJK_FAMILIES,
        SIMPLIFIED_CHINESE_CJK_FAMILIES, TRADITIONAL_CHINESE_CJK_FAMILIES,
    };

    #[test]
    fn preferred_cjk_families_defaults_to_simplified_chinese_for_non_cjk_locale() {
        assert_eq!(
            preferred_cjk_families_for_locale("en-US"),
            SIMPLIFIED_CHINESE_CJK_FAMILIES
        );
        assert_eq!(
            preferred_cjk_families_for_locale(""),
            SIMPLIFIED_CHINESE_CJK_FAMILIES
        );
    }

    #[test]
    fn preferred_cjk_families_respects_cjk_locale() {
        assert_eq!(
            preferred_cjk_families_for_locale("zh-CN"),
            SIMPLIFIED_CHINESE_CJK_FAMILIES
        );
        assert_eq!(
            preferred_cjk_families_for_locale("zh-Hans-US"),
            SIMPLIFIED_CHINESE_CJK_FAMILIES
        );
        assert_eq!(
            preferred_cjk_families_for_locale("zh-TW"),
            TRADITIONAL_CHINESE_CJK_FAMILIES
        );
        assert_eq!(
            preferred_cjk_families_for_locale("zh-Hant-HK"),
            TRADITIONAL_CHINESE_CJK_FAMILIES
        );
        assert_eq!(
            preferred_cjk_families_for_locale("ja-JP"),
            JAPANESE_CJK_FAMILIES
        );
        assert_eq!(
            preferred_cjk_families_for_locale("ko-KR"),
            KOREAN_CJK_FAMILIES
        );
    }
}
