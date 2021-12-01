use std::collections::HashMap;

use pdf_writer::types::{ColorSpace, MaskType, ShadingType};
use pdf_writer::writers::{ExtGraphicsState, Resources, ShadingPattern};
use pdf_writer::{Finish, Name, Rect, Ref};

use super::CoordToPdf;
use crate::render::Pattern;

pub struct PendingPattern {
    pub id: String,
    pub num: u32,
    pub shading_type: ShadingType,
    pub coords: [f32; 6],
    pub form_xobj: Option<Ref>,
}

impl PendingPattern {
    pub(crate) fn from_pattern(
        pattern: Pattern,
        bbox: usvg::Rect,
        num: u32,
        c: &CoordToPdf,
    ) -> Self {
        Self {
            coords: pattern.transformed_coords(c, bbox),
            id: pattern.id,
            num,
            shading_type: pattern.shading_type,
            form_xobj: None,
        }
    }
}

pub struct PendingGS {
    num: u32,
    stroke_opacity: Option<f32>,
    fill_opacity: Option<f32>,
    soft_mask: Option<Ref>,
}

impl PendingGS {
    fn new(num: u32) -> Self {
        Self {
            num,
            stroke_opacity: None,
            fill_opacity: None,
            soft_mask: None,
        }
    }

    pub fn soft_mask(smask: Ref, num: u32) -> Self {
        let mut res = Self::new(num);
        res.soft_mask = Some(smask);
        res
    }

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

    pub fn fill_opacity(opacity: f32, num: u32) -> Self {
        Self::opacity(None, Some(opacity), num)
    }
}

#[derive(Clone)]
pub struct PendingGroup {
    pub reference: Ref,
    pub bbox: Rect,
    pub matrix: Option<[f32; 6]>,
    pub initial_mask: Option<String>,
}

pub fn write_patterns(
    pending_patterns: &[PendingPattern],
    function_map: &HashMap<String, (Ref, Option<Ref>)>,
    resources: &mut Resources,
) {
    if pending_patterns.is_empty() {
        return;
    }

    let mut patterns = resources.key(Name(b"Pattern")).dict();

    for pending in pending_patterns.iter() {
        let name = format!("p{}", pending.num);
        let pattern_name = Name(name.as_bytes());
        let mut pattern = ShadingPattern::new(patterns.key(pattern_name));

        let func = function_map[&pending.id].0;

        let mut shading = pattern.shading();
        shading.shading_type(pending.shading_type);
        shading.color_space(ColorSpace::DeviceRgb);

        shading.function(func);
        shading.coords(IntoIterator::into_iter(pending.coords).take(
            if pending.shading_type == ShadingType::Axial {
                4
            } else {
                6
            },
        ));
        shading.extend([true, true]);
        shading.finish();

        if let Some(xobj) = pending.form_xobj {
            let mut ext_g = pattern.ext_graphics();
            let mut smask = ext_g.soft_mask();

            smask.subtype(MaskType::Luminosity);
            smask.group(xobj);
            smask.backdrop([0.0]);
        }
    }

    patterns.finish();
}

pub fn write_graphics(pending_graphics: &[PendingGS], resources: &mut Resources) {
    if pending_graphics.is_empty() {
        return;
    }

    let mut ext_gs = resources.key(Name(b"ExtGState")).dict();
    for gs in pending_graphics {
        let mut ext_g =
            ExtGraphicsState::new(ext_gs.key(Name(format!("gs{}", gs.num).as_bytes())));

        if let Some(stroke_opacity) = gs.stroke_opacity {
            ext_g.stroking_alpha(stroke_opacity);
        }

        if let Some(fill_opacity) = gs.fill_opacity {
            ext_g.non_stroking_alpha(fill_opacity);
        }

        if let Some(smask_id) = gs.soft_mask {
            let mut soft_mask = ext_g.soft_mask();
            soft_mask.subtype(MaskType::Luminosity);
            soft_mask.group(smask_id);
            soft_mask.finish();
        }
    }
    ext_gs.finish();
}

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
