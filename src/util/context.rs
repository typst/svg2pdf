use pdf_writer::{Chunk, Content, Filter, Ref};
use std::collections::{BTreeMap, HashMap};
use usvg::fontdb::ID;
use usvg::{fontdb, Group, ImageKind, Node, Tree};

use super::helper::deflate;
use crate::render::text::write_font;
use crate::util::allocate::RefAllocator;
use crate::{Options, GRAY_ICC_DEFLATED, SRGB_ICC_DEFLATED};

#[derive(Clone)]
pub struct Font {
    pub glyph_set: BTreeMap<u16, String>,
    pub reference: Ref,
    pub face_data: Vec<u8>,
    pub units_per_em: u16,
    pub face_index: u32,
}

/// Holds all of the necessary information for the conversion process.
pub struct Context {
    /// Options that where passed by the user.
    pub options: Options,
    /// The refs of the fonts
    pub fonts: HashMap<ID, Option<Font>>,
    srgb_ref: Option<Ref>,
    sgray_ref: Option<Ref>,
    pub ref_allocator: RefAllocator,
}

fn fill_fonts(group: &Group, ctx: &mut Context, fontdb: &fontdb::Database) {
    for child in group.children() {
        match child {
            Node::Text(t) => {
                let allocator = &mut ctx.ref_allocator;
                for span in t.layouted() {
                    for g in &span.positioned_glyphs {
                        let font = ctx.fonts.entry(g.font).or_insert_with(|| {
                            fontdb
                                .with_face_data(g.font, |data, face_index| {
                                    // TODO: Currently, we are parsing each font twice, once here
                                    // and once again when writing the fonts. We should probably
                                    // improve on that...
                                    if let Ok(ttf) =
                                        ttf_parser::Face::parse(data, face_index)
                                    {
                                        let reference = allocator.alloc_ref();
                                        let glyph_set = BTreeMap::new();
                                        return Some(Font {
                                            reference,
                                            face_data: Vec::from(data),
                                            units_per_em: ttf.units_per_em(),
                                            glyph_set,
                                            face_index,
                                        });
                                    }

                                    None
                                })
                                .flatten()
                        });

                        if let Some(ref mut font) = font {
                            font.glyph_set.insert(g.glyph_id.0, g.text.clone());
                        }
                    }
                }
            }
            Node::Group(group) => fill_fonts(group, ctx, fontdb),
            Node::Image(image) => {
                if let ImageKind::SVG(svg) = image.kind() {
                    fill_fonts(svg.root(), ctx, fontdb);
                }
            }
            _ => {}
        }

        child.subroots(|subroot| fill_fonts(subroot, ctx, fontdb));
    }
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree, options: Options, fontdb: &fontdb::Database) -> Self {
        let mut ctx = Self {
            ref_allocator: RefAllocator::new(),
            options,
            fonts: HashMap::new(),
            srgb_ref: None,
            sgray_ref: None,
        };

        fill_fonts(tree.root(), &mut ctx, fontdb);

        ctx
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

    pub fn font_ref(&self, id: ID) -> Option<&Font> {
        self.fonts.get(&id).and_then(|f| f.as_ref())
    }

    pub fn write_global_objects(&mut self, pdf: &mut Chunk) {
        let allocator = &mut self.ref_allocator;
        for font in self.fonts.values_mut() {
            if let Some(font) = font.as_mut() {
                write_font(pdf, allocator, font);
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
