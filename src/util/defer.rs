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
use pdf_writer::{Dict, Finish, Ref};

use crate::util::allocate::Allocator;
use crate::util::helper::{NameExt, SRGB};

#[derive(Clone, Copy, Eq, PartialEq)]
enum PendingObjectType {
    XObject,
    Pattern,
    GraphicsState,
    Shading,
}

impl PendingObjectType {
    fn get_name(&self, allocator: &mut Allocator) -> String {
        match *self {
            PendingObjectType::XObject => allocator.alloc_x_object_name(),
            PendingObjectType::Pattern => allocator.alloc_pattern_name(),
            PendingObjectType::GraphicsState => allocator.alloc_graphics_state_name(),
            PendingObjectType::Shading => allocator.alloc_shading_name(),
        }
    }

    fn get_dict<'a>(&'a self, resources: &'a mut Resources) -> Dict {
        match *self {
            PendingObjectType::XObject => resources.x_objects(),
            PendingObjectType::Pattern => resources.patterns(),
            PendingObjectType::GraphicsState => resources.ext_g_states(),
            PendingObjectType::Shading => resources.shadings(),
        }
    }

    pub fn iterator() -> impl Iterator<Item = PendingObjectType> {
        [
            PendingObjectType::XObject,
            PendingObjectType::Pattern,
            PendingObjectType::GraphicsState,
            PendingObjectType::Shading,
        ]
        .iter()
        .copied()
    }
}

struct Entry {
    object_type: PendingObjectType,
    reference: Ref,
    name: Rc<String>,
}

/// The actual struct that keeps track of deferred objects.
#[derive(Default)]
pub struct Deferrer {
    /// The allocator that allows us to allocate new Refs and Names.
    allocator: Allocator,
    /// The stack frames containing the deferred objects.
    pending_entries: Vec<Vec<Entry>>,
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

    /// Push a new stack frame.
    pub fn push(&mut self) {
        self.pending_entries.push(Vec::new());
    }

    /// Pop a stack frame and write the pending named resources into the `Resources` dictionary.
    pub fn pop(&mut self, resources: &mut Resources) {
        // TODO: Could probably be optimized? Not every XObject needs the color space entry.
        resources.color_spaces().insert(SRGB).start::<ColorSpace>().srgb();
        resources.proc_sets([ProcSet::Pdf, ProcSet::ImageColor, ProcSet::ImageGrayscale]);

        let entries = self.pending_entries.pop().unwrap();
        self.write_entries(resources, entries);
    }

    fn add_entry(
        &mut self,
        reference: Ref,
        object_type: PendingObjectType,
    ) -> Rc<String> {
        let name = Rc::new(object_type.get_name(&mut self.allocator));

        self.push_entry(Entry { object_type, reference, name: name.clone() });
        name
    }

    /// Add a new XObject entry. Returns the name of the XObject.
    pub fn add_x_object(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingObjectType::XObject)
    }

    /// Add a new Shading entry. Returns the name of the Shading.
    pub fn add_shading(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingObjectType::Shading)
    }

    /// Add a new Pattern entry. Returns the name of the Pattern.
    pub fn add_pattern(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingObjectType::Pattern)
    }

    /// Add a new GraphicsState entry. Returns the name of the GraphicsState.
    pub fn add_graphics_state(&mut self, reference: Ref) -> Rc<String> {
        self.add_entry(reference, PendingObjectType::GraphicsState)
    }

    /// Write all of the entries into a `Resources` dictionary.
    fn write_entries(&mut self, resources: &mut Resources, entries: Vec<Entry>) {
        for object_type in PendingObjectType::iterator() {
            let entries: Vec<_> =
                entries.iter().filter(|e| e.object_type == object_type).collect();

            if !entries.is_empty() {
                let mut dict = object_type.get_dict(resources);

                for entry in entries {
                    dict.pair(entry.name.as_name(), entry.reference);
                }

                dict.finish();
            }
        }
    }

    fn push_entry(&mut self, entry: Entry) {
        self.pending_entries.last_mut().unwrap().push(entry);
    }
}
