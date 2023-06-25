mod allocate;
mod defer;
pub mod helper;

use crate::util::helper::{calc_node_bbox, RectExt};
use defer::Deferrer;
use pdf_writer::Rect;
use usvg::utils::view_box_to_transform;
use usvg::{Node, Size, Transform, Tree, ViewBox};

#[derive(Clone)]
pub enum RenderContext {
    Normal,
    Pattern,
}

#[derive(Clone)]
struct Frame {
    render_context: RenderContext,
    current_transform: Transform,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            render_context: RenderContext::Normal,
            current_transform: Transform::default(),
        }
    }
}

pub struct ContextFrame {
    frames: Vec<Frame>,
    pub svg_base_transform: Transform,
}

impl ContextFrame {
    pub fn new(size: &Size, viewbox: &ViewBox) -> Self {
        let viewport_transform = Transform::new(1.0, 0.0, 0.0, -1.0, 0.0, size.height());
        let viewbox_transform =
            view_box_to_transform(viewbox.rect, viewbox.aspect, *size);

        let mut base_transform = viewport_transform;
        base_transform.append(&viewbox_transform);

        Self {
            frames: vec![Frame::default()],
            svg_base_transform: base_transform,
        }
    }

    fn current_frame(&self) -> &Frame {
        self.frames.last().unwrap()
    }

    pub fn set_render_context(&mut self, render_context: RenderContext) {
        self.current_frame_as_mut().render_context = render_context;
    }

    pub fn set_transform(&mut self, transform: Transform) {
        self.current_frame_as_mut().current_transform = transform;
    }

    fn current_frame_as_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    pub fn transform(&self) -> Transform {
        let mut base_transform = match self.current_frame().render_context {
            RenderContext::Normal => self.svg_base_transform,
            RenderContext::Pattern => Transform::default(),
        };

        base_transform.append(&self.raw_transform());
        base_transform
    }

    pub fn raw_transform(&self) -> Transform {
        self.current_frame().current_transform
    }

    pub fn push(&mut self) {
        self.frames.push(self.current_frame().clone());
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }

    pub fn append_transform(&mut self, transform: &Transform) {
        self.current_frame_as_mut().current_transform.append(transform);
    }
}

pub struct Context {
    pub viewbox: ViewBox,
    pub size: Size,
    pub deferrer: Deferrer,
    pub context_frame: ContextFrame,
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree) -> Self {
        Self {
            viewbox: tree.view_box,
            size: tree.size,
            deferrer: Deferrer::new(),
            context_frame: ContextFrame::new(&tree.size, &tree.view_box),
        }
    }

    pub fn get_media_box(&self) -> Rect {
        Rect::new(0.0, 0.0, self.size.width() as f32, self.size.height() as f32)
    }

    pub fn pdf_bbox(&self, node: &Node) -> Rect {
        match self.context_frame.current_frame().render_context {
            RenderContext::Normal => {
                self.pdf_bbox_with_transform(node, self.context_frame.raw_transform())
            }
            RenderContext::Pattern => self
                .svg_bbox_with_transform(node, self.context_frame.raw_transform())
                .as_pdf_rect(&Transform::default()),
        }
    }

    pub fn pdf_bbox_with_transform(&self, node: &Node, transform: Transform) -> Rect {
        self.svg_bbox_with_transform(node, transform)
            .as_pdf_rect(&self.context_frame.svg_base_transform)
    }

    pub fn svg_bbox_with_transform(
        &self,
        node: &Node,
        transform: Transform,
    ) -> usvg::Rect {
        calc_node_bbox(node, transform).and_then(|b| b.to_rect()).unwrap_or(
            usvg::Rect::new(0.0, 0.0, self.size.width(), self.size.height()).unwrap(),
        )
    }
}
