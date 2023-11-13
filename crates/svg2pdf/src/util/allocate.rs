/*! Allocate new object IDs and resource names.

For each object in a PDF document, a unique numerical ID needs to be assigned to it. The task of
the allocator is to keep track of the current ID. In addition to that, it allows us to generate
names that will be used for the named resources of an object.
 */

use pdf_writer::Ref;

/// The struct that holds all of the necessary counters.
pub struct Allocator {
    /// The next id for indirect object references.
    next_ref_id: i32,
    /// The next number that will be used for the name of an XObject in a resource
    /// dictionary, e.g. "xo0".
    next_x_object_num: i32,
    /// The next number that will be used for the name of a graphics state in a resource
    /// dictionary, e.g. "gs0".
    next_graphics_state_num: i32,
    /// The next number that will be used for the name of a pattern in a resource
    /// dictionary, e.g. "po0".
    next_patterns_num: i32,
    /// The next number that will be used for the name of a shading in a resource
    /// dictionary, e.g. "sh0".
    next_shadings_num: i32,
}

impl Default for Allocator {
    fn default() -> Self {
        Self {
            next_ref_id: 1,
            next_x_object_num: 0,
            next_graphics_state_num: 0,
            next_patterns_num: 0,
            next_shadings_num: 0,
        }
    }
}

impl Allocator {
    /// Manually set the next reference ID that should be used. Make sure that you only set it
    /// to a value that was higher than before, otherwise duplicate IDs might be assigned.
    pub fn set_next_ref(&mut self, next_ref: i32) {
        self.next_ref_id = next_ref
    }

    /// Create a new allocator with a specific start ID.
    pub fn new_with_start_ref(start_ref: i32) -> Self {
        Self { next_ref_id: start_ref, ..Allocator::default() }
    }

    /// Allocate a new reference ID.
    pub fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(self.next_ref_id);
        self.next_ref_id += 1;
        reference
    }

    /// Allocate a new XObject name.
    pub fn alloc_x_object_name(&mut self) -> String {
        let num = self.next_x_object_num;
        self.next_x_object_num += 1;
        format!("xo{}", num)
    }

    /// Allocate a new graphics state name.
    pub fn alloc_graphics_state_name(&mut self) -> String {
        let num = self.next_graphics_state_num;
        self.next_graphics_state_num += 1;
        format!("gs{}", num)
    }

    /// Allocate a new pattern name.
    pub fn alloc_pattern_name(&mut self) -> String {
        let num = self.next_patterns_num;
        self.next_patterns_num += 1;
        format!("po{}", num)
    }

    /// Allocate a new shading name.
    pub fn alloc_shading_name(&mut self) -> String {
        let num = self.next_shadings_num;
        self.next_shadings_num += 1;
        format!("sh{}", num)
    }
}
