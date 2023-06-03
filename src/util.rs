use pdf_writer::{Name, Rect, Ref};
use pdf_writer::types::ProcSet;
use pdf_writer::writers::{ColorSpace, Resources};
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

pub struct Context {
    next_id: i32,
    next_xobject: i32,
    dpi: f32,
    pub viewbox: ViewBox,
    pub size: Size,
    pub pending_xobjects: Vec<Vec<(String, Ref)>>
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
            pending_xobjects: Vec::new()
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

    /// Allocate a new indirect reference id.
    pub fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(self.next_id);
        self.next_id += 1;
        reference
    }

    fn alloc_xobject_num(&mut self) -> i32 {
        let xobject_num = self.next_xobject;
        self.next_xobject += 1;
        xobject_num
    }

    pub fn push_context(&mut self) {
        self.pending_xobjects.push(Vec::new());
    }

    pub fn pop_context(&mut self, resources: &mut Resources) {
        resources.color_spaces().insert(SRGB).start::<ColorSpace>().srgb();
        resources.proc_sets([ProcSet::Pdf, ProcSet::ImageColor, ProcSet::ImageGrayscale]);

        let pending_xobjects = self.pending_xobjects.pop().unwrap();

        if !pending_xobjects.is_empty() {
            let mut xobjects = resources.x_objects();
            for (name, ref_id) in pending_xobjects {
                xobjects.pair(Name(name.as_bytes()), ref_id);
            }
        }
    }

    pub fn alloc_xobject(&mut self) -> (String, Ref) {
        let object_ref = self.alloc_ref();
        let xobject_number = self.alloc_xobject_num();

        let name = format!("xo{}", xobject_number);
        let result =  (name, object_ref);

        self.pending_xobjects.last_mut().unwrap().push(result.clone());
        result
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