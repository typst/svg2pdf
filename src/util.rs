use crate::color::SRGB;
use pdf_writer::types::{MaskType, ProcSet};
use pdf_writer::writers::{ColorSpace, ExtGraphicsState, Resources};
use pdf_writer::{Finish, Name, Rect, Ref};
use usvg::{
    FuzzyEq, Node, NodeExt, NodeKind, PathBbox, PathData, Size, Transform, Tree, ViewBox,
};

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

pub struct Allocator {
    /// The next id for indirect object references
    next_ref_id: i32,
    /// The next number that will be used for the name of an XObject in a resource
    /// dictionary, e.g. "xo1"
    next_x_object_num: i32,
    /// The next number that will be used for the name of a graphics state in a resource
    /// dictionary, e.g. "gs1"
    next_graphics_state_num: i32,
}

pub struct PendingXObject {
    pub name: String,
    pub reference: Ref,
}

pub struct PendingGraphicsState {
    name: String,
    state_type: PendingGraphicsStateType,
}

enum PendingGraphicsStateType {
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
}

pub struct Deferrer {
    pending_x_objects: Vec<Vec<PendingXObject>>,
    pending_graphics_states: Vec<Vec<PendingGraphicsState>>,
}

impl Deferrer {
    pub fn new() -> Self {
        Deferrer {
            pending_x_objects: Vec::new(),
            pending_graphics_states: Vec::new(),
        }
    }

    pub fn push_context(&mut self) {
        self.pending_x_objects.push(Vec::new());
        self.pending_graphics_states.push(Vec::new());
    }

    pub fn pop_context(&mut self, resources: &mut Resources) {
        resources.color_spaces().insert(SRGB).start::<ColorSpace>().srgb();
        resources.proc_sets([ProcSet::Pdf, ProcSet::ImageColor, ProcSet::ImageGrayscale]);

        self.write_pending_x_objects(resources);
        self.write_pending_graphics_states(resources);
    }

    pub fn add_x_object(&mut self, name: String, reference: Ref) {
        self.pending_x_objects
            .last_mut()
            .unwrap()
            .push(PendingXObject { name, reference });
    }

    pub fn add_soft_mask(&mut self, name: String, group: Ref) {
        let state_type = PendingGraphicsStateType::SoftMask(SoftMask {
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
        let state_type = PendingGraphicsStateType::Opacity(Opacity {
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
                x_objects.pair(Name(x_object.name.as_bytes()), x_object.reference);
            }
            x_objects.finish();
        }
    }

    fn write_pending_graphics_states(&mut self, resources: &mut Resources) {
        let pending_graphics_states = self.pending_graphics_states.pop().unwrap();

        if !pending_graphics_states.is_empty() {
            let mut graphics = resources.ext_g_states();
            for pending_graphics_state in pending_graphics_states {
                let mut state = graphics
                    .insert(Name(pending_graphics_state.name.as_bytes()))
                    .start::<ExtGraphicsState>();

                match &pending_graphics_state.state_type {
                    PendingGraphicsStateType::SoftMask(soft_mask) => {
                        state
                            .soft_mask()
                            .subtype(soft_mask.mask_type)
                            .group(soft_mask.group)
                            .finish();
                    }
                    PendingGraphicsStateType::Opacity(opacity) => {
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

pub struct Context {
    dpi: f32,
    pub viewbox: ViewBox,
    pub size: Size,
    allocator: Allocator,
    deferrer: Deferrer,
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree) -> Self {
        Self {
            dpi: 72.0,
            viewbox: tree.view_box,
            size: tree.size,
            allocator: Allocator::new(),
            deferrer: Deferrer::new(),
        }
    }

    pub fn dpi_factor(&self) -> f32 {
        72.0 / self.dpi
    }

    pub fn get_media_box(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            self.size.width() as f32 * self.dpi_factor(),
            self.size.height() as f32 * self.dpi_factor(),
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
}

// Taken from resvg
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
