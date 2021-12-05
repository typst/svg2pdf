//! Defer the writing of some data structures.
//!
//! Many data structures are associated with the `Resources` dictionary of a
//! page or a Form XObject. This module contains the structs to queue them up
//! and functions to ultimately populate them to the file.

use std::collections::HashMap;

use pdf_writer::types::{MaskType, ShadingType};
use pdf_writer::writers::{ExtGraphicsState, Resources, ShadingPattern};
use pdf_writer::{Name, Rect, Ref};

use super::CoordToPdf;
use crate::render::Gradient;

/// A gradient to be written.
///
/// In PDF parlance, a gradient is a type of pattern and is specific to its
/// dimensions.
pub struct PendingGradient {
    /// The unique SVG id of the pattern which is used to fetch the associated
    /// pattern function.
    pub id: String,
    /// The number allocated by [`Context::alloc_pattern`] for reference in
    /// content streams as e.g. `p4`.
    pub num: u32,
    /// How the gradient shading is distributed in its area.
    pub shading_type: ShadingType,
    /// The coordinates of where to apply the gradient within the content
    /// stream's bounding box. Note that the last two components are zero for
    /// radial gradients.
    pub coords: [f32; 6],
}

impl PendingGradient {
    /// Create a new instance from a pattern property struct.
    pub(crate) fn from_gradient(
        pattern: Gradient,
        bbox: usvg::Rect,
        num: u32,
        c: &CoordToPdf,
    ) -> Self {
        Self {
            coords: pattern.transformed_coords(c, bbox),
            id: pattern.id,
            num,
            shading_type: pattern.shading_type,
        }
    }
}

/// A graphics state to be written.
///
/// Currently, svg2pdf mostly uses graphics state dictionaries to encode
/// transparency data.
pub struct PendingGS {
    /// The number allocated by [`Context::alloc_gs`] for reference in
    /// content streams as e.g. `gs3`.
    num: u32,
    /// The opacity of strokes within the current drawing state.
    stroke_opacity: Option<f32>,
    /// The opacity of fill operations within the current drawing state.
    fill_opacity: Option<f32>,
    /// An indirect reference to a Soft Mask, which is associated with another
    /// content stream that dictates the alpha value for the whole bounding box.
    ///
    /// Here, the indirect reference is expected to refer to an Form XObject
    /// that is used in Luminosity mode.
    soft_mask: Option<Ref>,
}

impl PendingGS {
    /// Create a new, empty pending graphics state.
    fn new(num: u32) -> Self {
        Self {
            num,
            stroke_opacity: None,
            fill_opacity: None,
            soft_mask: None,
        }
    }

    /// Create a pending graphics state which will set a luminosity Soft Mask
    /// with the referenced Form XObject.
    pub fn soft_mask(smask: Ref, num: u32) -> Self {
        let mut res = Self::new(num);
        res.soft_mask = Some(smask);
        res
    }

    /// Create a pending graphics state which will set the stroke and fill
    /// opacity for its drawing state.
    pub fn opacity(
        stroke_opacity: Option<f32>,
        fill_opacity: Option<f32>,
        num: u32,
    ) -> Self {
        let mut res = Self::new(num);
        res.stroke_opacity = stroke_opacity;
        res.fill_opacity = fill_opacity;
        res
    }

    /// Create a pending graphics state which will set the fill opacity for its
    /// drawing state.
    pub fn fill_opacity(opacity: f32, num: u32) -> Self {
        Self::opacity(None, Some(opacity), num)
    }
}

/// Store metadata for transparency group Form XObjects that must be written at
/// some other point because the top-level writer is currently not available.
///
/// This is distinct from the `write_xobjects` method in the sense that that
/// method only registers Form XObjects with the `Resources` dictionary, but the
/// XObjects themselves have already been written. The transparency groups for
/// this struct, however, are not automatically registered as a resource.
///
/// These structs are used in [`Context`], where they are stored together with
/// an ID string that identifies an element in the SVG's def section. At some
/// point, the content stream for that element is created and the Form XObject
/// is populated with this metadata.
#[derive(Clone)]
pub struct PendingGroup {
    /// The indirect reference that has been pre-allocated for the Form XObject.
    pub reference: Ref,
    /// The PDF bounding box of the form XObject.
    pub bbox: Rect,
    /// A transformation matrix to allow for a different coordinate system use
    /// in the object.
    pub matrix: Option<[f32; 6]>,
    /// An SVG ID to a mask that should be applied at the start of the content
    /// stream.
    pub initial_mask: Option<String>,
}

/// Writes all pending gradients and patterns into a `Resources` dictionary. The
/// gradient functions do not depend on the dimensions of the element they are
/// applied to, are written at the start of the conversion process, and
/// therefore the `function_map` retains their references.
pub fn write_gradients(
    pending_gradients: &[PendingGradient],
    pending_patterns: &[(u32, Ref)],
    function_map: &HashMap<String, (Ref, Option<Ref>)>,
    resources: &mut Resources,
) {
    if pending_gradients.is_empty() && pending_patterns.is_empty() {
        return;
    }

    let mut patterns = resources.patterns();

    for pending in pending_gradients.iter() {
        let name = format!("p{}", pending.num);
        let mut pattern =
            patterns.insert(Name(name.as_bytes())).start::<ShadingPattern>();

        // The object has already been outfitted with an alpha soft mask, so we
        // can disregard the alpha function option.
        let func = function_map[&pending.id].0;

        let mut shading = pattern.shading();
        shading.shading_type(pending.shading_type);
        shading.color_space().srgb();
        shading.function(func);
        shading.coords(IntoIterator::into_iter(pending.coords).take(
            if pending.shading_type == ShadingType::Axial {
                4
            } else {
                6
            },
        ));
        shading.extend([true, true]);
    }

    for (num, ref_id) in pending_patterns {
        let name = format!("p{}", num);
        patterns.pair(Name(name.as_bytes()), *ref_id);
    }
}

/// Writes all pending graphics states into a `Resources` dictionary.
pub fn write_graphics(pending_graphics: &[PendingGS], resources: &mut Resources) {
    if pending_graphics.is_empty() {
        return;
    }

    // PdfWriter's `Resources::ext_g_states` method requires an indirect
    // reference to the graphics state dictionaries. We cannot, however, at this
    // point create a new indirect object since the top-level writer is already
    // mutably borrowed through the `Resources` writer. We resort to direct
    // objects instead.
    let mut states = resources.ext_g_states();
    for gs in pending_graphics {
        let mut state = states
            .insert(Name(format!("gs{}", gs.num).as_bytes()))
            .start::<ExtGraphicsState>();

        if let Some(stroke_opacity) = gs.stroke_opacity {
            state.stroking_alpha(stroke_opacity);
        }

        if let Some(fill_opacity) = gs.fill_opacity {
            state.non_stroking_alpha(fill_opacity);
        }

        if let Some(smask_id) = gs.soft_mask {
            state.soft_mask().subtype(MaskType::Luminosity).group(smask_id);
        }
    }
}

/// Register indirect XObjects with the `Resources` dictionary such that they
/// can be used as `xo123` in content streams.
pub fn write_xobjects(pending_xobjects: &[(u32, Ref)], resources: &mut Resources) {
    if pending_xobjects.is_empty() {
        return;
    }

    let mut xobjects = resources.x_objects();
    for (num, ref_id) in pending_xobjects {
        let name = format!("xo{}", num);
        xobjects.pair(Name(name.as_bytes()), *ref_id);
    }
}
