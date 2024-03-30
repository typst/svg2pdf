use pdf_writer::{Chunk, Content, Filter, Ref};

#[cfg(feature = "text")]
use {
    crate::render::text,
    crate::render::text::{write_font, Font},
    std::collections::HashMap,
    usvg::fontdb,
    usvg::fontdb::ID,
    usvg::Tree,
};

use super::helper::deflate;
use crate::util::allocate::RefAllocator;
use crate::{Options, GRAY_ICC_DEFLATED, SRGB_ICC_DEFLATED};

/// Holds all of the necessary information for the conversion process.
pub struct Context {
    /// Options that where passed by the user.
    pub options: Options,
    /// The refs of the fonts
    #[cfg(feature = "text")]
    pub fonts: HashMap<ID, Option<Font>>,
    srgb_ref: Option<Ref>,
    sgray_ref: Option<Ref>,
    pub ref_allocator: RefAllocator,
}

impl Context {
    /// Create a new context.
    #[cfg(feature = "text")]
    pub fn new(tree: &Tree, options: Options, fontdb: &fontdb::Database) -> Self {
        let mut ctx = Self {
            ref_allocator: RefAllocator::new(),
            options,
            fonts: HashMap::new(),
            srgb_ref: None,
            sgray_ref: None,
        };

        if options.embed_text {
            text::fill_fonts(tree.root(), &mut ctx, fontdb);
        }

        ctx
    }

    // TODO: Make context less ugly with different features.
    /// Create a new context.
    #[cfg(not(feature = "text"))]
    pub fn new(options: Options) -> Self {
        Self {
            ref_allocator: RefAllocator::new(),
            options,
            srgb_ref: None,
            sgray_ref: None,
        }
    }

    /// Allocate a new reference.
    pub fn alloc_ref(&mut self) -> Ref {
        self.ref_allocator.alloc_ref()
    }

    pub fn srgb_ref(&mut self) -> Ref {
        let alloc = &mut self.ref_allocator;
        let srgb_ref = &mut self.srgb_ref;

        *srgb_ref.get_or_insert_with(|| alloc.alloc_ref())
    }

    pub fn sgray_ref(&mut self) -> Ref {
        let alloc = &mut self.ref_allocator;
        let sgray_ref = &mut self.sgray_ref;

        *sgray_ref.get_or_insert_with(|| alloc.alloc_ref())
    }

    #[cfg(feature = "text")]
    pub fn font_ref(&self, id: ID) -> Option<&Font> {
        self.fonts.get(&id).and_then(|f| f.as_ref())
    }

    pub fn write_global_objects(&mut self, pdf: &mut Chunk) {
        #[cfg(feature = "text")]
        {
            let allocator = &mut self.ref_allocator;

            for font in self.fonts.values_mut() {
                if let Some(font) = font.as_mut() {
                    write_font(pdf, allocator, font);
                }
            }
        }

        if let Some(srgb_ref) = self.srgb_ref {
            pdf.icc_profile(srgb_ref, &SRGB_ICC_DEFLATED)
                .n(3)
                .range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0])
                .filter(Filter::FlateDecode);
        }

        if let Some(sgray_ref) = self.sgray_ref {
            pdf.icc_profile(sgray_ref, &GRAY_ICC_DEFLATED)
                .n(1)
                .range([0.0, 1.0])
                .filter(Filter::FlateDecode);
        }
    }

    /// Just a helper method so that we don't have to manually compress the content if this was
    /// set in the [Options] struct.
    pub fn finish_content(&self, content: Content) -> Vec<u8> {
        if self.options.compress {
            deflate(&content.finish())
        } else {
            content.finish()
        }
    }
}
