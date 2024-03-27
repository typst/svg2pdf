/*! The context holds information that we might need access to while converting the SVG.

This includes information like for example what the size of the SVG is, whether content should be
compressed and access to an instance of the deferrer + allocator.
*/

use owned_ttf_parser::{AsFaceRef, OwnedFace};
use pdf_writer::{Content, Ref};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use usvg::fontdb::{Database, ID};
use usvg::{fontdb, Group, ImageKind, Node, NonZeroRect, Size, Transform, Tree, ViewBox};

use super::defer::Deferrer;
use super::helper::deflate;
use crate::Options;

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
    /// The view box of the SVG.
    pub view_box: ViewBox,
    /// The size of the SVG.
    pub size: Size,
    /// An instance of the deferrer.
    pub deferrer: Deferrer,
    /// Options that where passed by the user.
    pub options: Options,
    /// The refs of the fonts
    pub fonts: HashMap<ID, Font>,
    pub fontdb: Database,
}

fn fill_fonts<'a>(group: &Group, ctx: &mut Context, fontdb: &fontdb::Database) {
    for child in group.children() {
        match child {
            Node::Text(t) => {
                for span in t.layouted() {
                    for g in &span.positioned_glyphs {
                        if !ctx.fonts.contains_key(&g.font) {
                            fontdb.with_face_data(g.font, |data, face_index| {
                                let data = Vec::from(data);
                                let reference = ctx.alloc_ref();
                                let face = OwnedFace::from_vec(data.clone(), face_index)
                                    .unwrap();
                                let glyph_set = BTreeMap::new();
                                ctx.fonts.insert(
                                    g.font,
                                    Font {
                                        reference,
                                        face_data: data,
                                        units_per_em: face.as_face_ref().units_per_em(),
                                        glyph_set,
                                        face_index,
                                    },
                                )
                            });
                        }

                        let font = ctx.fonts.get_mut(&g.font).unwrap();
                        font.glyph_set.insert(g.glyph_id.0, g.text.clone());
                    }
                }
            }
            Node::Group(group) => fill_fonts(group, ctx, fontdb),
            Node::Image(image) => match image.kind() {
                ImageKind::SVG(svg) => fill_fonts(svg.root(), ctx, fontdb),
                _ => {}
            },
            _ => {}
        }

        child.subroots(|subroot| fill_fonts(subroot, ctx, fontdb));
    }
}

impl Context {
    /// Create a new context.
    pub fn new(
        tree: &Tree,
        options: Options,
        start_ref: Option<i32>,
        fontdb: fontdb::Database,
    ) -> Self {
        let mut ctx = Self {
            view_box: tree.view_box(),
            size: tree.size(),
            deferrer: Deferrer::new_with_start_ref(start_ref.unwrap_or(1)),
            options,
            fonts: HashMap::new(),
            fontdb: fontdb.clone(),
        };

        fill_fonts(tree.root(), &mut ctx, &fontdb);

        ctx
    }

    /// Allocate a new reference.
    pub fn alloc_ref(&mut self) -> Ref {
        self.deferrer.alloc_ref()
    }

    /// Get the base transform that needs to be applied before rendering everything else (
    /// i.e. the initial transform passed by the user + the view box transform to account for the
    /// view box of the SVG).
    pub fn get_view_box_transform(&self) -> Transform {
        self.view_box.to_transform(self.size)
    }

    /// Returns a [`usvg` Rect](usvg::Rect) with the dimensions of the whole SVG.
    pub fn get_rect(&self) -> NonZeroRect {
        NonZeroRect::from_xywh(0.0, 0.0, self.size.width(), self.size.height()).unwrap()
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
