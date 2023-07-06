use crate::util::defer::Deferrer;
use crate::util::helper::{calc_node_bbox, dpi_ratio};
use crate::Options;
use pdf_writer::Rect;
use usvg::utils::view_box_to_transform;
use usvg::{Node, Size, Transform, Tree, ViewBox};

pub struct Context {
    pub viewbox: ViewBox,
    pub size: Size,
    pub initial_transform: Transform,
    pub deferrer: Deferrer,
    pub options: Options,
}

impl Context {
    /// Create a new context.
    pub fn new(
        tree: &Tree,
        options: Options,
        initial_transform: Transform,
        start_ref: Option<i32>,
    ) -> Self {
        Self {
            viewbox: tree.view_box,
            size: tree.size,
            initial_transform,
            deferrer: Deferrer::new_with_start_ref(start_ref.unwrap_or(1)),
            options,
        }
    }

    pub fn get_base_transform(&self) -> Transform {
        let viewbox_transform =
            view_box_to_transform(self.viewbox.rect, self.viewbox.aspect, self.size);

        let mut base_transform = self.initial_transform;
        base_transform.append(&viewbox_transform);
        base_transform
    }

    pub fn get_media_box(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            self.size.width() as f32 * dpi_ratio(self.options.dpi),
            self.size.height() as f32 * dpi_ratio(self.options.dpi),
        )
    }

    pub fn plain_bbox(&self, node: &Node) -> usvg::Rect {
        calc_node_bbox(node, Transform::default())
            .and_then(|b| b.to_rect())
            .unwrap_or(
                usvg::Rect::new(
                    0.0,
                    0.0,
                    (self.size.width() as f32 * dpi_ratio(self.options.dpi)) as f64,
                    (self.size.height() as f32 * dpi_ratio(self.options.dpi)) as f64,
                )
                .unwrap(),
            )
    }
}
