use pdf_writer::{Finish, Name, Rect, Ref};
use pdf_writer::types::{MaskType, ProcSet};
use pdf_writer::writers::{ColorSpace, ExtGraphicsState, Reference, Resources};
use usvg::{Tree, ViewBox, Size, Node, Transform, PathBbox, NodeKind, PathData, NodeExt, FuzzyEq};
use crate::color::SRGB;

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
    next_graphics_state_num: i32
}

pub struct PendingXObject {
    pub name: String,
    pub reference: Ref
}

pub struct PendingGraphicsState {
    pub name: String,
    pub mask_type: MaskType,
    pub group: Ref
}

impl Allocator {
    pub fn new() -> Self {
        Self {
            next_ref_id: 1,
            next_x_object_num: 0,
            next_graphics_state_num: 0
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
        let num = self.next_x_object_num;
        self.next_x_object_num += 1;
        format!("gs{}", num)
    }
}

pub struct Deferrer {
    pending_x_objects: Vec<Vec<PendingXObject>>,
    pending_graphics_states: Vec<Vec<PendingGraphicsState>>
}

impl Deferrer {
    pub fn new() -> Self {
        Deferrer {
            pending_x_objects: Vec::new(),
            pending_graphics_states: Vec::new()
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
        self.pending_x_objects.last_mut().unwrap().push(PendingXObject {name, reference});
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
                let mut state = graphics.insert(Name(pending_graphics_state.name.as_bytes())).start::<ExtGraphicsState>();
                state.soft_mask().subtype(MaskType::Alpha).group(pending_graphics_state.group);
                state.finish();
            }
            graphics.finish();
        }
    }
}

pub struct Context {
    next_id: i32,
    next_xobject: i32,
    dpi: f32,
    pub viewbox: ViewBox,
    pub size: Size,
    allocator: Allocator,
    deferrer: Deferrer
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree) -> Self {
        Self {
            next_id: 1,
            next_xobject: 0,
            dpi: 72.0,
            viewbox: tree.view_box,
            size: tree.size,
            allocator: Allocator::new(),
            deferrer: Deferrer::new()
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
}


// Taken from resvg
pub fn calc_node_bbox(node: &Node, ts: Transform) -> Option<PathBbox> {
    match *node.borrow() {
        NodeKind::Path(ref path) => path.data.bbox_with_transform(ts, path.stroke.as_ref()),
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