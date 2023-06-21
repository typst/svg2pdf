use crate::color::SRGB;
use pdf_writer::types::{MaskType, ProcSet};
use pdf_writer::writers::{ColorSpace, ExtGraphicsState, Resources};
use pdf_writer::{Finish, Name, Rect, Ref};
use usvg::{FuzzyEq, Node, NodeExt, NodeKind, PathBbox, PathData, Point, Size, Transform, Tree, ViewBox};
use usvg::utils::view_box_to_transform;

pub trait TransformExt {
    fn get_transform(&self) -> [f32; 6];
}

impl TransformExt for usvg::Transform {
    fn get_transform(&self) -> [f32; 6] {
        [
            self.a as f32,
            self.b as f32,
            self.c as f32,
            self.d as f32,
            self.e as f32,
            self.f as f32,
        ]
    }
}

pub trait NameExt {
    fn as_name(&self) -> Name;
}

impl NameExt for String {
    fn as_name(&self) -> Name {
        Name(self.as_bytes())
    }
}

pub struct Allocator {
    /// The next id for indirect object references
    next_ref_id: i32,
    /// The next number that will be used for the name of an XObject in a resource
    /// dictionary, e.g. "xo0"
    next_x_object_num: i32,
    /// The next number that will be used for the name of a graphics state in a resource
    /// dictionary, e.g. "gs0"
    next_graphics_state_num: i32,
    /// The next number that will be used for the name of a pattern in a resource
    /// dictionary, e.g. "po0"
    next_patterns_num: i32,
}

pub struct PendingXObject {
    pub name: String,
    pub reference: Ref,
}

pub struct PendingPattern {
    pub name: String,
    pub reference: Ref,
}

pub struct PendingGraphicsState {
    name: String,
    state_type: GraphicsStateType,
}

enum GraphicsStateType {
    Opacity(Opacity),
    SoftMask(SoftMask),
}

struct Opacity {
    stroke_opacity: f32,
    fill_opacity: f32,
}

struct SoftMask {
    mask_type: MaskType,
    group: Ref,
}

impl Allocator {
    pub fn new() -> Self {
        Self {
            next_ref_id: 1,
            next_x_object_num: 0,
            next_graphics_state_num: 0,
            next_patterns_num: 0
        }
    }

    pub fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(self.next_ref_id);
        self.next_ref_id += 1;
        reference
    }

    pub fn alloc_x_object_name(&mut self) -> String {
        let num = self.next_x_object_num;
        self.next_x_object_num += 1;
        format!("xo{}", num)
    }

    pub fn alloc_graphics_state_name(&mut self) -> String {
        let num = self.next_graphics_state_num;
        self.next_graphics_state_num += 1;
        format!("gs{}", num)
    }

    pub fn alloc_patterns_name(&mut self) -> String {
        let num = self.next_patterns_num;
        self.next_patterns_num += 1;
        format!("po{}", num)
    }
}

pub struct Deferrer {
    pending_x_objects: Vec<Vec<PendingXObject>>,
    pending_patterns: Vec<Vec<PendingPattern>>,
    pending_graphics_states: Vec<Vec<PendingGraphicsState>>,
}

impl Deferrer {
    pub fn new() -> Self {
        Deferrer {
            pending_x_objects: Vec::new(),
            pending_graphics_states: Vec::new(),
            pending_patterns: Vec::new()
        }
    }

    pub fn push_context(&mut self) {
        self.pending_x_objects.push(Vec::new());
        self.pending_patterns.push(Vec::new());
        self.pending_graphics_states.push(Vec::new());
    }

    pub fn pop_context(&mut self, resources: &mut Resources) {
        resources.color_spaces().insert(SRGB).start::<ColorSpace>().srgb();
        resources.proc_sets([ProcSet::Pdf, ProcSet::ImageColor, ProcSet::ImageGrayscale]);

        self.write_pending_x_objects(resources);
        self.write_pending_graphics_states(resources);
        self.write_pending_patterns(resources);
    }

    pub fn add_x_object(&mut self, name: String, reference: Ref) {
        self.pending_x_objects
            .last_mut()
            .unwrap()
            .push(PendingXObject { name, reference });
    }

    pub fn add_pattern(&mut self, name: String, reference: Ref) {
        self.pending_patterns
            .last_mut()
            .unwrap()
            .push(PendingPattern { name, reference });
    }

    pub fn add_soft_mask(&mut self, name: String, group: Ref) {
        let state_type = GraphicsStateType::SoftMask(SoftMask {
            mask_type: MaskType::Alpha,
            group,
        });
        self.pending_graphics_states
            .last_mut()
            .unwrap()
            .push(PendingGraphicsState { name, state_type });
    }

    pub fn add_opacity(
        &mut self,
        name: String,
        stroke_opacity: Option<f32>,
        fill_opacity: Option<f32>,
    ) {
        let state_type = GraphicsStateType::Opacity(Opacity {
            stroke_opacity: stroke_opacity.unwrap_or(1.0),
            fill_opacity: fill_opacity.unwrap_or(1.0),
        });

        self.pending_graphics_states
            .last_mut()
            .unwrap()
            .push(PendingGraphicsState { name, state_type });
    }

    fn write_pending_x_objects(&mut self, resources: &mut Resources) {
        let pending_x_objects = self.pending_x_objects.pop().unwrap();

        if !pending_x_objects.is_empty() {
            let mut x_objects = resources.x_objects();
            for x_object in pending_x_objects {
                x_objects.pair(x_object.name.as_name(), x_object.reference);
            }
            x_objects.finish();
        }
    }

    fn write_pending_patterns(&mut self, resources: &mut Resources) {
        let pending_patterns = self.pending_patterns.pop().unwrap();

        if !pending_patterns.is_empty() {
            let mut patterns = resources.patterns();
            for pattern in pending_patterns {
                patterns.pair(pattern.name.as_name(), pattern.reference);
            }
            patterns.finish();
        }
    }

    fn write_pending_graphics_states(&mut self, resources: &mut Resources) {
        let pending_graphics_states = self.pending_graphics_states.pop().unwrap();

        if !pending_graphics_states.is_empty() {
            let mut graphics = resources.ext_g_states();
            for pending_graphics_state in pending_graphics_states {
                let mut state = graphics
                    .insert(pending_graphics_state.name.as_name())
                    .start::<ExtGraphicsState>();

                match &pending_graphics_state.state_type {
                    GraphicsStateType::SoftMask(soft_mask) => {
                        state
                            .soft_mask()
                            .subtype(soft_mask.mask_type)
                            .group(soft_mask.group)
                            .finish();
                    }
                    GraphicsStateType::Opacity(opacity) => {
                        state
                            .non_stroking_alpha(opacity.fill_opacity)
                            .stroking_alpha(opacity.stroke_opacity)
                            .finish();
                    }
                }
            }
            graphics.finish();
        }
    }
}

#[derive(Clone)]
pub enum RenderContext {
    SVG
}

#[derive(Clone)]
struct Frame {
    render_context: RenderContext,
    current_transform: Transform
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            render_context: RenderContext::SVG,
            current_transform: Transform::default(),
        }
    }
}

pub struct ContextFrame {
    frames: Vec<Frame>,
    pub svg_base_transform: Transform
}

impl ContextFrame {
    pub fn new(size: &Size, viewbox: &ViewBox) -> Self {
        let viewport_transform = Transform::new(1.0, 0.0, 0.0, -1.0, 0.0, size.height());
        let viewbox_transform = view_box_to_transform(viewbox.rect, viewbox.aspect, *size);

        let mut base_transform = viewport_transform;
        base_transform.append(&viewbox_transform);

        Self {
            frames: vec![Frame::default()],
            svg_base_transform: base_transform
        }
    }

    fn current_frame(&self) -> &Frame {
        self.frames.last().unwrap()
    }

    fn current_frame_as_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    pub fn transform(&self) -> Transform {
        let mut base_transform = match self.current_frame().render_context {
            RenderContext::SVG => self.svg_base_transform
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
    pub context_frame: ContextFrame
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree) -> Self {
        Self {
            viewbox: tree.view_box,
            size: tree.size,
            allocator: Allocator::new(),
            deferrer: Deferrer::new(),
            context_frame: ContextFrame::new(&tree.size, &tree.view_box)
        }
    }

    pub fn get_media_box(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            self.size.width() as f32,
            self.size.height() as f32,
        )
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
        let name = self.allocator.alloc_patterns_name();

        self.deferrer.add_pattern(name.clone(), reference);
        (name, reference)
    }

    pub fn alloc_soft_mask(&mut self, group: Ref) -> String {
        let name = self.allocator.alloc_graphics_state_name();

        self.deferrer.add_soft_mask(name.clone(), group);
        name.clone()
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
            let mut bottom_right = Point { x: rect.x() + rect.width(), y: rect.y() + rect.height() };
            self.context_frame.svg_base_transform.apply_to(&mut top_left.x, &mut top_left.y);
            self.context_frame.svg_base_transform.apply_to(&mut bottom_right.x, &mut bottom_right.y);

            //left, bottom, right, top
            Rect {x1: top_left.x as f32, y1: bottom_right.y as f32, x2: bottom_right.x as f32, y2: top_left.y as f32 }
        }).unwrap_or(self.get_media_box())
    }

    pub fn pdf_bbox(&self, node: &Node) -> Rect {
        let opt_rect = calc_node_bbox(node, self.context_frame.raw_transform())
            .and_then(|b| b.to_rect());
        self.pdf_bbox_from_rect(opt_rect.as_ref())
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