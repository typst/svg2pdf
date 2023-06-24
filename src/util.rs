pub mod helper;
mod allocate;
mod defer;

use crate::util::helper::SRGB;

use pdf_writer::types::{MaskType, ProcSet};
use pdf_writer::writers::{ColorSpace, ExtGraphicsState, Resources};
use pdf_writer::{Finish, Rect, Ref};
use usvg::utils::view_box_to_transform;
use usvg::{
    FuzzyEq, Node, NodeExt, NodeKind, PathBbox, PathData, Point, Size, Transform, Tree,
    ViewBox,
};
use allocate::Allocator;
use defer::Deferrer;
use helper::NameExt;

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
    allocator: Allocator,
    deferrer: Deferrer,
    pub context_frame: ContextFrame,
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree) -> Self {
        Self {
            viewbox: tree.view_box,
            size: tree.size,
            allocator: Allocator::new(),
            deferrer: Deferrer::new(),
            context_frame: ContextFrame::new(&tree.size, &tree.view_box),
        }
    }

    pub fn get_media_box(&self) -> Rect {
        Rect::new(0.0, 0.0, self.size.width() as f32, self.size.height() as f32)
    }

    pub fn push_context(&mut self) {
        self.deferrer.push_context();
    }

    pub fn pop_context(&mut self, resources: &mut Resources) {
        self.deferrer.pop_context(resources);
    }

    pub fn alloc_ref(&mut self) -> Ref {
        self.allocator.alloc_ref()
    }

    pub fn alloc_named_x_object(&mut self) -> (String, Ref) {
        let reference = self.alloc_ref();
        let name = self.allocator.alloc_x_object_name();

        self.deferrer.add_x_object(name.clone(), reference);
        (name, reference)
    }

    pub fn alloc_named_pattern(&mut self) -> (String, Ref) {
        let reference = self.alloc_ref();
        let name = self.allocator.alloc_pattern_object_name();

        self.deferrer.add_pattern(name.clone(), reference);
        (name, reference)
    }

    pub fn alloc_soft_mask(&mut self, group: Ref) -> String {
        let name = self.allocator.alloc_graphics_state_name();

        self.deferrer.add_soft_mask(name.clone(), group);
        name
    }

    pub fn alloc_opacity(
        &mut self,
        stroke_opacity: Option<f32>,
        fill_opacity: Option<f32>,
    ) -> String {
        let name = self.allocator.alloc_graphics_state_name();

        self.deferrer.add_opacity(name.clone(), stroke_opacity, fill_opacity);
        name
    }

    pub fn pdf_bbox_from_rect(&self, rect: Option<&usvg::Rect>) -> Rect {
        rect.map(|rect| {
            let mut top_left = Point { x: rect.x(), y: rect.y() };
            let mut bottom_right = Point {
                x: rect.x() + rect.width(),
                y: rect.y() + rect.height(),
            };
            self.context_frame
                .svg_base_transform
                .apply_to(&mut top_left.x, &mut top_left.y);
            self.context_frame
                .svg_base_transform
                .apply_to(&mut bottom_right.x, &mut bottom_right.y);

            //left, bottom, right, top
            Rect {
                x1: top_left.x as f32,
                y1: bottom_right.y as f32,
                x2: bottom_right.x as f32,
                y2: top_left.y as f32,
            }
        })
        .unwrap_or(self.get_media_box())
    }

    pub fn pdf_bbox(&self, node: &Node) -> Rect {
        match self.context_frame.current_frame().render_context {
            RenderContext::Normal => {
                let opt_rect = calc_node_bbox(node, self.context_frame.raw_transform())
                    .and_then(|b| b.to_rect());
                self.pdf_bbox_from_rect(opt_rect.as_ref())
            }
            RenderContext::Pattern => {
                let opt_rect = calc_node_bbox(node, self.context_frame.transform())
                    .and_then(|b| b.to_rect())
                    .unwrap();
                Rect::new(
                    opt_rect.x() as f32,
                    opt_rect.y() as f32,
                    (opt_rect.x() + opt_rect.width()) as f32,
                    (opt_rect.y() + opt_rect.height()) as f32,
                )
            }
        }
    }

    pub fn usvg_rect_to_pdf_rect(&self, rect: &usvg::Rect) -> Rect {
        let transformed = rect.transform(&self.context_frame.transform()).unwrap();
        Rect::new(
            transformed.x() as f32,
            transformed.y() as f32,
            (transformed.x() + transformed.width()) as f32,
            (transformed.y() + transformed.height()) as f32,
        )
    }
}

pub fn calc_node_bbox(node: &Node, ts: Transform) -> Option<PathBbox> {
    match *node.borrow() {
        NodeKind::Path(ref path) => {
            path.data.bbox_with_transform(ts, path.stroke.as_ref())
        }
        NodeKind::Image(ref img) => {
            let path = PathData::from_rect(img.view_box.rect);
            path.bbox_with_transform(ts, None)
        }
        NodeKind::Group(_) => {
            let mut bbox = PathBbox::new_bbox();

            for child in node.children() {
                let mut child_transform = ts;
                child_transform.append(&child.transform());
                if let Some(c_bbox) = calc_node_bbox(&child, child_transform) {
                    bbox = bbox.expand(c_bbox);
                }
            }

            // Make sure bbox was changed.
            if bbox.fuzzy_eq(&PathBbox::new_bbox()) {
                return None;
            }

            Some(bbox)
        }
        NodeKind::Text(_) => None,
    }
}
