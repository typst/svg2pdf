use crate::util::defer::Deferrer;
use crate::util::helper::calc_node_bbox;
use pdf_writer::Rect;
use usvg::utils::view_box_to_transform;
use usvg::{Node, Size, Transform, Tree, ViewBox};

#[derive(Clone)]
#[derive(Default)]
pub struct Frame {
    base_transform: Transform,
    current_transform: Transform,
}

impl Frame {
    pub fn full_transform(&self) -> Transform {
        let mut transform = self.base_transform;
        transform.append(&self.current_transform);
        transform
    }
}

pub struct ContextFrame {
    frames: Vec<Frame>,
}

impl ContextFrame {
    pub fn new() -> Self {
        Self { frames: vec![Frame::default()] }
    }

    fn current_frame(&self) -> &Frame {
        self.frames.last().unwrap()
    }

    pub fn set_base_transform(&mut self, transform: Transform) {
        self.current_frame_as_mut().base_transform = transform;
    }

    fn current_frame_as_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    pub fn full_transform(&self) -> Transform {
        self.current_frame().full_transform()
    }

    pub fn push(&mut self) {
        self.frames.push(self.current_frame().clone());
    }

    pub fn push_new(&mut self) {
        self.frames.push(Frame::default());
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
        let mut context = Self {
            viewbox: tree.view_box,
            size: tree.size,
            deferrer: Deferrer::new(),
            context_frame: ContextFrame::new(),
        };

        let viewport_transform =
            Transform::new(1.0, 0.0, 0.0, -1.0, 0.0, context.size.height());
        let viewbox_transform = view_box_to_transform(
            context.viewbox.rect,
            context.viewbox.aspect,
            context.size,
        );

        let mut base_transform = viewport_transform;
        base_transform.append(&viewbox_transform);

        context.context_frame.set_base_transform(base_transform);
        context
    }

    pub fn get_media_box(&self) -> Rect {
        Rect::new(0.0, 0.0, self.size.width() as f32, self.size.height() as f32)
    }

    pub fn plain_bbox(&self, node: &Node) -> usvg::Rect {
        calc_node_bbox(node, Transform::default())
            .and_then(|b| b.to_rect())
            .unwrap_or(
                usvg::Rect::new(0.0, 0.0, self.size.width(), self.size.height()).unwrap(),
            )
    }
}
