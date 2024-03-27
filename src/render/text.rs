use crate::render::path::{draw_path, set_opacity_gs};
use crate::render::{gradient, group, path, pattern};
use crate::util::context::{Context, Font};
use crate::util::defer::SRGB;
use crate::util::helper::{
    deflate, ColorExt, LineCapExt, LineJoinExt, NameExt, TransformExt,
};
use owned_ttf_parser::{name_id, AsFaceRef, Face, GlyphId, OwnedFace, PlatformId, Tag};
use pdf_writer::types::ColorSpaceOperand::Pattern;
use pdf_writer::types::{
    CidFontType, ColorSpaceOperand, FontFlags, SystemInfo, TextRenderingMode, UnicodeCmap,
};
use pdf_writer::{Chunk, Content, Filter, Finish, Name, Ref, Str};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use tiny_skia::Transform;
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
use usvg::fontdb::ID;
use usvg::layout::Span;
use usvg::{Fill, Paint, PaintOrder, Stroke, Visibility};

const CFF: Tag = Tag::from_bytes(b"CFF ");
const CFF2: Tag = Tag::from_bytes(b"CFF2");
const CMAP_NAME: Name = Name(b"Custom");
const SYSTEM_INFO: SystemInfo = SystemInfo {
    registry: Str(b"Adobe"),
    ordering: Str(b"Identity"),
    supplement: 0,
};

pub fn write_font(chunk: &mut Chunk, ctx: &mut Context, id: ID) -> Option<()> {
    let mut font = ctx.fonts.remove(&id).unwrap();

    let owned_ttf = OwnedFace::from_vec(font.face_data, font.face_index).unwrap();
    let ttf = owned_ttf.as_face_ref();
    let units_per_em = ttf.units_per_em();

    let type0_ref = font.reference;
    let cid_ref = ctx.alloc_ref();
    let descriptor_ref = ctx.alloc_ref();
    let cmap_ref = ctx.alloc_ref();
    let data_ref = ctx.alloc_ref();

    let glyph_set = &mut font.glyph_set;

    // Do we have a TrueType or CFF font?
    //
    // FIXME: CFF2 must be handled differently and requires PDF 2.0
    // (or we have to convert it to CFF).
    let is_cff = ttf
        .raw_face()
        .table(CFF)
        .or_else(|| ttf.raw_face().table(CFF2))
        .is_some();

    let postscript_name = find_name(&ttf, name_id::POST_SCRIPT_NAME)
        .unwrap_or_else(|| "unknown".to_string());

    let subset_tag = subset_tag();
    let base_font = format!("{subset_tag}+{postscript_name}");
    let base_font_type0 =
        if is_cff { format!("{base_font}-Identity-H") } else { base_font.clone() };

    chunk
        .type0_font(type0_ref)
        .base_font(Name(base_font_type0.as_bytes()))
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_ref)
        .to_unicode(cmap_ref);

    // Write the CID font referencing the font descriptor.
    let mut cid = chunk.cid_font(cid_ref);
    cid.subtype(if is_cff { CidFontType::Type0 } else { CidFontType::Type2 });
    cid.base_font(Name(base_font.as_bytes()));
    cid.system_info(SYSTEM_INFO);
    cid.font_descriptor(descriptor_ref);
    cid.default_width(0.0);
    if !is_cff {
        cid.cid_to_gid_map_predefined(Name(b"Identity"));
    }

    let mut widths = vec![];
    for gid in std::iter::once(0).chain(glyph_set.keys().copied()) {
        let width = ttf.glyph_hor_advance(GlyphId(gid)).unwrap_or(0);
        let units = (width as f64 / units_per_em as f64) * 1000.0;
        let cid = glyph_cid(&ttf, gid);
        if usize::from(cid) >= widths.len() {
            widths.resize(usize::from(cid) + 1, 0.0);
            widths[usize::from(cid)] = units as f32;
        }
    }

    // Write all non-zero glyph widths.
    let mut first = 0;
    let mut width_writer = cid.widths();
    for (w, group) in widths.group_by_key(|&w| w) {
        let end = first + group.len();
        if w != 0.0 {
            let last = end - 1;
            width_writer.same(first as u16, last as u16, w);
        }
        first = end;
    }

    width_writer.finish();
    cid.finish();

    let mut flags = FontFlags::empty();
    flags.set(FontFlags::SERIF, postscript_name.contains("Serif"));
    flags.set(FontFlags::FIXED_PITCH, ttf.is_monospaced());
    flags.set(FontFlags::ITALIC, ttf.is_italic());
    flags.insert(FontFlags::SYMBOLIC);
    flags.insert(FontFlags::SMALL_CAP);

    let global_bbox = ttf.global_bounding_box();
    let bbox = pdf_writer::Rect::new(
        (global_bbox.x_min as f32 / units_per_em as f32) * 1000.0,
        (global_bbox.y_min as f32 / units_per_em as f32) * 1000.0,
        (global_bbox.x_max as f32 / units_per_em as f32) * 1000.0,
        (global_bbox.y_max as f32 / units_per_em as f32) * 1000.0,
    );

    let italic_angle = ttf.italic_angle().unwrap_or(0.0);
    let ascender = ttf.typographic_ascender().unwrap_or(ttf.ascender());
    let descender = ttf.typographic_descender().unwrap_or(ttf.descender());
    let cap_height = ttf.capital_height().filter(|&h| h > 0).unwrap_or(ascender);
    let stem_v = 10.0 + 0.244 * (f32::from(ttf.weight().to_number()) - 50.0);

    // Write the font descriptor (contains metrics about the font).
    let mut font_descriptor = chunk.font_descriptor(descriptor_ref);
    font_descriptor
        .name(Name(base_font.as_bytes()))
        .flags(flags)
        .bbox(bbox)
        .italic_angle(italic_angle)
        .ascent(ascender as f32)
        .descent(descender as f32)
        .cap_height(cap_height as f32)
        .stem_v(stem_v);

    font_descriptor.font_file2(data_ref);
    font_descriptor.finish();

    let cmap = create_cmap(ttf, glyph_set);
    chunk.cmap(cmap_ref, &cmap.finish());

    // Subset and write the font's bytes.
    let glyphs: Vec<_> = glyph_set.keys().copied().collect();
    let data = subset_font(owned_ttf.as_slice(), font.face_index, &glyphs);

    let mut stream = chunk.stream(data_ref, &data);
    stream.filter(Filter::FlateDecode);
    if is_cff {
        stream.pair(Name(b"Subtype"), Name(b"CIDFontType0C"));
    }

    stream.finish();
    Some(())
}

/// Create a /ToUnicode CMap.
fn create_cmap(ttf: &Face, glyph_set: &mut BTreeMap<u16, String>) -> UnicodeCmap {
    // For glyphs that have codepoints mapping to them in the font's cmap table,
    // we prefer them over pre-existing text mappings from the document. Only
    // things that don't have a corresponding codepoint (or only a private-use
    // one) like the "Th" in Linux Libertine get the text of their first
    // occurrences in the document instead.
    for subtable in ttf.tables().cmap.into_iter().flat_map(|table| table.subtables) {
        if !subtable.is_unicode() {
            continue;
        }

        subtable.codepoints(|n| {
            let Some(c) = std::char::from_u32(n) else { return };
            if c.general_category() == GeneralCategory::PrivateUse {
                return;
            }

            let Some(GlyphId(g)) = ttf.glyph_index(c) else { return };
            if glyph_set.contains_key(&g) {
                glyph_set.insert(g, c.into());
            }
        });
    }

    // Produce a reverse mapping from glyphs' CIDs to unicode strings.
    let mut cmap = UnicodeCmap::new(CMAP_NAME, SYSTEM_INFO);
    for (&g, text) in glyph_set.iter() {
        if !text.is_empty() {
            cmap.pair_with_multiple(glyph_cid(ttf, g), text.chars());
        }
    }

    cmap
}

fn subset_font(font_data: &[u8], index: u32, glyphs: &[u16]) -> Vec<u8> {
    let data = font_data;
    let profile = subsetter::Profile::pdf(glyphs);
    let subsetted = subsetter::subset(data, index, profile);
    let mut data = subsetted.as_deref().unwrap_or(data);

    // Extract the standalone CFF font program if applicable.

    let face = owned_ttf_parser::RawFace::parse(data, 0).unwrap();
    if let Some(cff) = face.table(CFF) {
        data = cff;
    }

    deflate(data)
}

pub fn render(
    text: &usvg::Text,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    let mut font_names = HashMap::new();
    let ctx_fonts = ctx.fonts.clone();

    for span in text.layouted() {
        for glyph in &span.positioned_glyphs {
            if let Some(font) = ctx_fonts.get(&glyph.font) {
                if !font_names.contains_key(&font.reference) {
                    font_names
                        .insert(font.reference, ctx.deferrer.add_font(font.reference));
                }
            }
        }
    }

    for span in text.layouted() {
        if span.visibility != Visibility::Visible {
            continue;
        }

        let operation = |content: &mut Content| {
            for glyph in &span.positioned_glyphs {
                if let Some(font) = ctx_fonts.get(&glyph.font).cloned() {
                    let name = font_names.get(&font.reference).unwrap();

                    let gid = glyph.glyph_id.0;
                    let ts = glyph
                        .transform
                        .pre_scale(font.units_per_em as f32, font.units_per_em as f32)
                        // The glyphs in usvg are already scaled according the font size, but we
                        // we want to leverage the native PDF font size feature instead, so we downscale
                        // it to a font size of 1.
                        .pre_scale(
                            1.0 / span.font_size.get(),
                            1.0 / span.font_size.get(),
                        );
                    content.save_state();
                    content.begin_text();
                    content.set_text_matrix(ts.to_pdf_transform());
                    content.set_font(Name(name.as_bytes()), span.font_size.get());

                    content.show(Str(&[(gid >> 8) as u8, (gid & 0xff) as u8]));

                    content.end_text();
                    content.restore_state();
                }
            }
        };

        let stroke_operation = |content: &mut Content, stroke: &Stroke| {
            content.set_text_rendering_mode(TextRenderingMode::Stroke);
            operation(content);
        };

        let fill_operation = |content: &mut Content, fill: &Fill| {
            content.set_text_rendering_mode(TextRenderingMode::Fill);
            operation(content);
        };

        if let Some(overline) = &span.overline {
            path::render(overline, chunk, content, ctx, accumulated_transform);
        }

        if let Some(underline) = &span.underline {
            path::render(underline, chunk, content, ctx, accumulated_transform);
        }

        content.save_state();
        match (span.fill.as_ref(), span.stroke.as_ref()) {
            (Some(fill), Some(stroke)) => match span.paint_order {
                PaintOrder::FillAndStroke => {
                    path::fill(
                        fill,
                        chunk,
                        content,
                        ctx,
                        fill_operation,
                        accumulated_transform,
                    );
                    path::stroke(
                        stroke,
                        chunk,
                        content,
                        ctx,
                        stroke_operation,
                        accumulated_transform,
                    );
                }
                PaintOrder::StrokeAndFill => {
                    path::stroke(
                        stroke,
                        chunk,
                        content,
                        ctx,
                        stroke_operation,
                        accumulated_transform,
                    );
                    path::fill(
                        fill,
                        chunk,
                        content,
                        ctx,
                        fill_operation,
                        accumulated_transform,
                    );
                }
            },
            (None, Some(stroke)) => {
                path::stroke(
                    stroke,
                    chunk,
                    content,
                    ctx,
                    stroke_operation,
                    accumulated_transform,
                );
            }
            (Some(fill), None) => {
                path::fill(
                    fill,
                    chunk,
                    content,
                    ctx,
                    fill_operation,
                    accumulated_transform,
                );
            }
            // TODO: Should actually be invisible text
            (None, None) => {}
        };

        content.restore_state();

        if let Some(line_through) = &span.line_through {
            path::render(line_through, chunk, content, ctx, accumulated_transform);
        }
    }
}

/// Produce a unique 6 letter tag for a glyph set.
fn subset_tag() -> String {
    // TODO: Actually implement it
    "AAAAAA".to_string()
    // const LEN: usize = 6;
    // const BASE: u128 = 26;
    // // TODO: Randomize
    // let mut hash = typst::util::hash128(&glyphs);
    // let mut letter = [b'A'; LEN];
    // for l in letter.iter_mut() {
    //     *l = b'A' + (hash % BASE) as u8;
    //     hash /= BASE;
    // }
    // std::str::from_utf8(&letter).unwrap().into()
}

/// Try to find and decode the name with the given id.
pub(super) fn find_name(ttf: &Face, name_id: u16) -> Option<String> {
    ttf.names().into_iter().find_map(|entry| {
        if entry.name_id == name_id {
            if let Some(string) = entry.to_string() {
                return Some(string);
            }

            if entry.platform_id == PlatformId::Macintosh && entry.encoding_id == 0 {
                return Some(decode_mac_roman(entry.name));
            }
        }

        None
    })
}

/// Decode mac roman encoded bytes into a string.
fn decode_mac_roman(coded: &[u8]) -> String {
    #[rustfmt::skip]
    const TABLE: [char; 128] = [
        'Ä', 'Å', 'Ç', 'É', 'Ñ', 'Ö', 'Ü', 'á', 'à', 'â', 'ä', 'ã', 'å', 'ç', 'é', 'è',
        'ê', 'ë', 'í', 'ì', 'î', 'ï', 'ñ', 'ó', 'ò', 'ô', 'ö', 'õ', 'ú', 'ù', 'û', 'ü',
        '†', '°', '¢', '£', '§', '•', '¶', 'ß', '®', '©', '™', '´', '¨', '≠', 'Æ', 'Ø',
        '∞', '±', '≤', '≥', '¥', 'µ', '∂', '∑', '∏', 'π', '∫', 'ª', 'º', 'Ω', 'æ', 'ø',
        '¿', '¡', '¬', '√', 'ƒ', '≈', '∆', '«', '»', '…', '\u{a0}', 'À', 'Ã', 'Õ', 'Œ', 'œ',
        '–', '—', '“', '”', '‘', '’', '÷', '◊', 'ÿ', 'Ÿ', '⁄', '€', '‹', '›', 'ﬁ', 'ﬂ',
        '‡', '·', '‚', '„', '‰', 'Â', 'Ê', 'Á', 'Ë', 'È', 'Í', 'Î', 'Ï', 'Ì', 'Ó', 'Ô',
        '\u{f8ff}', 'Ò', 'Ú', 'Û', 'Ù', 'ı', 'ˆ', '˜', '¯', '˘', '˙', '˚', '¸', '˝', '˛', 'ˇ',
    ];

    fn char_from_mac_roman(code: u8) -> char {
        if code < 128 {
            code as char
        } else {
            TABLE[(code - 128) as usize]
        }
    }

    coded.iter().copied().map(char_from_mac_roman).collect()
}

fn glyph_cid(ttf: &Face, glyph_id: u16) -> u16 {
    ttf.tables()
        .cff
        .and_then(|cff| cff.glyph_cid(GlyphId(glyph_id)))
        .unwrap_or(glyph_id)
}

/// Extra methods for [`[T]`](slice).
pub trait SliceExt<T> {
    /// Split a slice into consecutive runs with the same key and yield for
    /// each such run the key and the slice of elements with that key.
    fn group_by_key<K, F>(&self, f: F) -> GroupByKey<'_, T, F>
    where
        F: FnMut(&T) -> K,
        K: PartialEq;
}

impl<T> SliceExt<T> for [T] {
    fn group_by_key<K, F>(&self, f: F) -> GroupByKey<'_, T, F> {
        GroupByKey { slice: self, f }
    }
}

/// This struct is created by [`SliceExt::group_by_key`].
pub struct GroupByKey<'a, T, F> {
    slice: &'a [T],
    f: F,
}

impl<'a, T, K, F> Iterator for GroupByKey<'a, T, F>
where
    F: FnMut(&T) -> K,
    K: PartialEq,
{
    type Item = (K, &'a [T]);

    fn next(&mut self) -> Option<Self::Item> {
        let mut iter = self.slice.iter();
        let key = (self.f)(iter.next()?);
        let count = 1 + iter.take_while(|t| (self.f)(t) == key).count();
        let (head, tail) = self.slice.split_at(count);
        self.slice = tail;
        Some((key, head))
    }
}
