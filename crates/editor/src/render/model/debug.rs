use std::fmt;

use sum_tree::SumTree;

use super::{BlockItem, LayoutSummary, RenderState};

/// Extension trait for types with verbose descriptive formatting.
pub trait Describe {
    /// Describe this item into the given formatter.
    fn describe_to(&self, f: &mut fmt::Formatter) -> fmt::Result;

    /// Describe this item.
    fn describe(&self) -> Description<'_, Self> {
        Description(self)
    }
}

pub struct Description<'a, T: ?Sized>(&'a T);

impl<T: Describe + ?Sized> fmt::Display for Description<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.describe_to(f)
    }
}

impl Describe for RenderState {
    fn describe_to(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let viewport_start = self.viewport.scroll_top().as_f32() as f64;
        let viewport_end = viewport_start + self.viewport.height().as_f32() as f64;
        writeln!(f, "Viewport: {viewport_start:.2}px to {viewport_end:.2}px")?;
        writeln!(f, "Selection: {}", self.selections())?;

        let mut in_viewport = false;
        let content = self.content.borrow();
        let mut cursor = content.cursor::<(), LayoutSummary>();
        cursor.descend_to_first_item(&content, |_| true);
        while let Some(item) = cursor.item() {
            let start_summary = cursor.start();

            let item_start = start_summary.height;
            let item_end = item_start + item.height().as_f32() as f64;

            // Is this the end of the viewport?
            if item_start > viewport_end && in_viewport {
                in_viewport = false;
                writeln!(f, "============> VIEWPORT END <============")?;
            }

            // Is this the start of the viewport?
            if item_end >= viewport_start && item_start <= viewport_end && !in_viewport {
                in_viewport = true;
                writeln!(f, "============> VIEWPORT START <============")?;
            }

            writeln!(
                f,
                "-------- {:.2}px / {} characters --------",
                start_summary.height, start_summary.content_length
            )?;
            writeln!(f, "  {}", item.describe())?;
            cursor.next();
        }

        if in_viewport {
            writeln!(f, "============> VIEWPORT END <============")?;
        }

        Ok(())
    }
}

impl Describe for SumTree<BlockItem> {
    fn describe_to(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut cursor = self.cursor::<(), LayoutSummary>();
        cursor.descend_to_first_item(self, |_| true);
        while let Some(item) = cursor.item() {
            let summary = cursor.start();
            writeln!(
                f,
                "-------- {:.2}px / {} characters --------\n{}",
                summary.height,
                summary.content_length,
                item.describe()
            )?;
            cursor.next();
        }
        Ok(())
    }
}

impl Describe for BlockItem {
    fn describe_to(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Paragraph(_) => f.write_str("Paragraph")?,
            Self::TextBlock { .. } => f.write_str("Text Block")?,
            Self::RunnableCodeBlock {
                code_block_type, ..
            } => write!(f, "Code Block - {code_block_type}",)?,
            Self::MermaidDiagram { .. } => f.write_str("Mermaid Diagram")?,
            Self::TemporaryBlock { .. } => f.write_str("Temporary Paragraph")?,
            Self::TaskList {
                indent_level,
                complete,
                ..
            } => write!(
                f,
                "Task List @ {indent_level} [{}]",
                if *complete { "X" } else { " " }
            )?,
            Self::UnorderedList { indent_level, .. } => {
                write!(f, "Unordered List @ {indent_level}")?
            }
            Self::OrderedList { indent_level, .. } => write!(f, "Ordered List @ {indent_level}")?,
            Self::Header { header_size, .. } => write!(f, "{header_size:?}")?,
            Self::HorizontalRule(_) => f.write_str("Horizontal Rule")?,
            Self::Image { alt_text, .. } => write!(f, "Image: {alt_text}")?,
            Self::Table(laid_out_table) => write!(
                f,
                "Table: {}x{}",
                laid_out_table.table.rows.len() + 1,
                laid_out_table.table.headers.len()
            )?,
            Self::TrailingNewLine(_) => f.write_str("Trailing Newline")?,
            Self::Embedded(_) => f.write_str("Embedded Item")?,
            Self::Hidden { .. } => f.write_str("Hidden")?,
        }

        write!(
            f,
            " ({} characters, {} lines, {:.2}px tall)",
            self.content_length(),
            self.lines(),
            self.height()
        )
    }
}
