use crate::util::allocate::Allocator;
use crate::util::helper::{NameExt, SRGB};
use pdf_writer::types::{MaskType, ProcSet};
use pdf_writer::writers::{ColorSpace, ExtGraphicsState, Resources};
use pdf_writer::{Finish, Ref};

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

pub struct Deferrer {
    allocator: Allocator,
    pending_x_objects: Vec<Vec<PendingXObject>>,
    pending_patterns: Vec<Vec<PendingPattern>>,
    pending_graphics_states: Vec<Vec<PendingGraphicsState>>,
}

impl Deferrer {
    pub fn new() -> Self {
        Deferrer {
            allocator: Allocator::new(),
            pending_x_objects: Vec::new(),
            pending_graphics_states: Vec::new(),
            pending_patterns: Vec::new(),
        }
    }

    pub fn push(&mut self) {
        self.pending_x_objects.push(Vec::new());
        self.pending_patterns.push(Vec::new());
        self.pending_graphics_states.push(Vec::new());
    }

    pub fn pop(&mut self, resources: &mut Resources) {
        resources.color_spaces().insert(SRGB).start::<ColorSpace>().srgb();
        resources.proc_sets([ProcSet::Pdf, ProcSet::ImageColor, ProcSet::ImageGrayscale]);

        self.write_pending_x_objects(resources);
        self.write_pending_graphics_states(resources);
        self.write_pending_patterns(resources);
    }

    pub fn alloc_ref(&mut self) -> Ref {
        self.allocator.alloc_ref()
    }

    pub fn add_x_object(&mut self) -> (String, Ref) {
        let reference = self.alloc_ref();
        let name = self.allocator.alloc_x_object_name();

        self.push_x_object(name.clone(), reference);
        (name, reference)
    }

    pub fn add_pattern(&mut self) -> (String, Ref) {
        let reference = self.alloc_ref();
        let name = self.allocator.alloc_pattern_object_name();

        self.push_pattern(name.clone(), reference);
        (name, reference)
    }

    pub fn add_soft_mask(&mut self, group: Ref) -> String {
        let name = self.allocator.alloc_graphics_state_name();

        self.push_soft_mask(name.clone(), group);
        name
    }

    pub fn add_opacity(
        &mut self,
        stroke_opacity: Option<f32>,
        fill_opacity: Option<f32>,
    ) -> String {
        let name = self.allocator.alloc_graphics_state_name();

        self.push_opacity(name.clone(), stroke_opacity, fill_opacity);
        name
    }

    fn push_x_object(&mut self, name: String, reference: Ref) {
        self.pending_x_objects
            .last_mut()
            .unwrap()
            .push(PendingXObject { name, reference });
    }

    fn push_pattern(&mut self, name: String, reference: Ref) {
        self.pending_patterns
            .last_mut()
            .unwrap()
            .push(PendingPattern { name, reference });
    }

    fn push_soft_mask(&mut self, name: String, group: Ref) {
        let state_type =
            GraphicsStateType::SoftMask(SoftMask { mask_type: MaskType::Alpha, group });
        self.pending_graphics_states
            .last_mut()
            .unwrap()
            .push(PendingGraphicsState { name, state_type });
    }

    fn push_opacity(
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
