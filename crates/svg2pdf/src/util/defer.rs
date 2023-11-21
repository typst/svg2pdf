/*! Defer the writing of some data structures.

While traversing the `resvg` tree, we will sometimes have to add certain objects to the resource
dictionary of the XObject that we are currently writing. However, due to the nature of how
`pdf_writer` works, we have to first finish writing the whole content stream before we can update
the `Resources` dictionary of the XObject. Because of this, we need the [Deferrer]: The [Deferrer]
is a stack-like structure that allows us to push and pop new "frames" that contain all of the
named resources that needed to be created while generating the content stream for that XObject.

Once we are done writing the whole content stream, we can just pop the deferrer and then
add all of the named resources to the `Resources` dictionary of the XObject.
*/

use std::rc::Rc;

use pdf_writer::types::ProcSet;
use pdf_writer::writers::{ColorSpace, Resources};
use pdf_writer::{Dict, Finish, Name, Ref};

use super::allocate::Allocator;
use super::helper::NameExt;

pub const SRGB: Name = Name(b"srgb");

#[derive(Clone, Copy, Eq, PartialEq)]
enum PendingResourceType {
    XObject,
    Pattern,
    GraphicsState,
    Shading,
}

impl PendingResourceType {
    fn get_name(&self, allocator: &mut Allocator) -> String {
        match *self {
            PendingResourceType::XObject => allocator.alloc_x_object_name(),
            PendingResourceType::Pattern => allocator.alloc_pattern_name(),
            PendingResourceType::GraphicsState => allocator.alloc_graphics_state_name(),
            PendingResourceType::Shading => allocator.alloc_shading_name(),
        }
    }

    fn get_dict<'a>(&'a self, resources: &'a mut Resources) -> Dict {
        match *self {
            PendingResourceType::XObject => resources.x_objects(),
            PendingResourceType::Pattern => resources.patterns(),
            PendingResourceType::GraphicsState => resources.ext_g_states(),
            PendingResourceType::Shading => resources.shadings(),
        }
    }

    pub fn iterator() -> impl Iterator<Item = PendingResourceType> {
        [
            PendingResourceType::XObject,
            PendingResourceType::Pattern,
            PendingResourceType::GraphicsState,
            PendingResourceType::Shading,
        ]
        .iter()
        .copied()
    }
}

struct PendingResource {
    object_type: PendingResourceType,
    reference: Ref,
    name: Rc<String>,
}

/// The actual struct that keeps track of deferred objects.
#[derive(Default)]
pub struct Deferrer {
    /// The allocator that allows us to allocate new Refs and Names.
    allocator: Allocator,
    /// The stack frames containing the deferred objects.
    pending_entries: Vec<Vec<PendingResource>>,
    /// The reference to the icc profile for srgb.
    srgb_ref: Option<Ref>,
    // The reference to the icc profile for sgray
    sgray_ref: Option<Ref>,
}

impl Deferrer {
    /// Create a new deferrer with an specific start reference ID for the allocator.
    pub fn new_with_start_ref(start_ref: i32) -> Self {
        Self {
            allocator: Allocator::new_with_start_ref(start_ref),
            ..Deferrer::default()
        }
    }

    /// Set the next reference ID of the allocator. WARNING: Only use this to INCREASE
    /// the reference counter (unless you know what you're doing), otherwise you might cause
    /// the allocator to assign two objects the same reference.
    pub fn set_next_ref(&mut self, next_ref: i32) {
        self.allocator.set_next_ref(next_ref);
    }

    /// Allocate a new reference through the allocator.
    pub fn alloc_ref(&mut self) -> Ref {
        self.allocator.alloc_ref()
    }

    pub fn used_srgb(&self) -> bool {
        self.srgb_ref.is_some()
    }

    pub fn used_sgray(&self) -> bool {
        self.sgray_ref.is_some()
    }

    pub fn srgb_ref(&mut self) -> Ref {
        let allocator = &mut self.allocator;
        *self.srgb_ref.get_or_insert_with(|| allocator.alloc_ref())
    }

    pub fn sgray_ref(&mut self) -> Ref {
        let allocator = &mut self.allocator;
        *self.sgray_ref.get_or_insert_with(|| allocator.alloc_ref())
    }

    /// Push a new stack frame.
    pub fn push(&mut self) {
        self.pending_entries.push(Vec::new());
    }

    /// Pop a stack frame and write the pending named resources into the `Resources` dictionary.
    pub fn pop(&mut self, resources: &mut Resources) {
        let mut color_spaces = resources.color_spaces();
        color_spaces
            .insert(SRGB)
            .start::<ColorSpace>()
            .icc_based(self.srgb_ref());
        // SGRAY is currently only used for soft masks with alpha, so we never need to write
        // it into the resources directly.
        color_spaces.finish();
        resources.proc_sets([ProcSet::Pdf, ProcSet::ImageColor, ProcSet::ImageGrayscale]);

        let entries = self.pending_entries.pop().unwrap();
        self.write_entries(resources, entries);
    }

    fn add_entry(
        &mut self,
        reference: Ref,
        object_type: PendingResourceType,
    ) -> Rc<String> {
        let name = Rc::new(object_type.get_name(&mut self.allocator));

        self.push_entry(PendingResource { object_type, reference, name: name.clone() });
        name
    }

    /// Add a new XObject entry. Returns the name of the XObject.
    pub fn add_x_object(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingResourceType::XObject)
    }

    /// Add a new Shading entry. Returns the name of the Shading.
    pub fn add_shading(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingResourceType::Shading)
    }

    /// Add a new Pattern entry. Returns the name of the Pattern.
    pub fn add_pattern(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingResourceType::Pattern)
    }

    /// Add a new GraphicsState entry. Returns the name of the GraphicsState.
    pub fn add_graphics_state(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingResourceType::GraphicsState)
    }

    /// Write all of the entries into a `Resources` dictionary.
    fn write_entries(
        &mut self,
        resources: &mut Resources,
        entries: Vec<PendingResource>,
    ) {
        for object_type in PendingResourceType::iterator() {
            let entries: Vec<_> =
                entries.iter().filter(|e| e.object_type == object_type).collect();

            if !entries.is_empty() {
                let mut dict = object_type.get_dict(resources);

                for entry in entries {
                    dict.pair(entry.name.to_pdf_name(), entry.reference);
                }

                dict.finish();
            }
        }
    }

    fn push_entry(&mut self, entry: PendingResource) {
        self.pending_entries.last_mut().unwrap().push(entry);
    }
}
