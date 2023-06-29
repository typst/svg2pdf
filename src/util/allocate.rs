use pdf_writer::Ref;

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

impl Default for Allocator {
    fn default() -> Self {
        Self {
            next_ref_id: 1,
            next_x_object_num: 0,
            next_graphics_state_num: 0,
            next_patterns_num: 0,
        }
    }
}

impl Allocator {

    pub fn new_with_start_ref(start_ref: i32) -> Self {
        Self {
            next_ref_id: start_ref,
            ..Allocator::default()
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

    pub fn alloc_pattern_object_name(&mut self) -> String {
        let num = self.next_patterns_num;
        self.next_patterns_num += 1;
        format!("po{}", num)
    }
}
