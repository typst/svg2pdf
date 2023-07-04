use std::rc::Rc;
use pdf_writer::{Finish, Name, PdfWriter, Ref, Writer};
use pdf_writer::types::ShadingType;
use pdf_writer::writers::ExponentialFunction;
use usvg::StopOffset;
use crate::util::context::Context;
use crate::util::helper::{ColorExt, RectExt, TransformExt};

pub fn create_linear(
    gradient: Rc<usvg::LinearGradient>,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    let (pattern_name, pattern_id) = ctx.deferrer.add_pattern();
    let shading_function = get_shading_function(gradient, writer, ctx);
    let mut shading_pattern = writer.shading_pattern(pattern_id);
    let mut shading = shading_pattern.shading();
    shading.shading_type(ShadingType::Axial);
    shading.color_space().srgb();

    shading.function(shading_function);
    shading.insert(Name(b"Domain")).array().items([0.0, 1.0]);

    let coords_rect = parent_bbox.as_pdf_rect(&ctx.context_frame.full_transform());
    // TODO: Figure out the proper values for y
    shading.coords([coords_rect.x1, coords_rect.y2, coords_rect.x2, coords_rect.y2]);
    shading.finish();

    shading_pattern.finish();

    pattern_name
}

fn get_spread_shading_function(
    gradient: Rc<usvg::LinearGradient>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
    ctx.deferrer.alloc_ref()
}

fn get_shading_function(
    gradient: Rc<usvg::LinearGradient>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
    let mut stops = gradient.stops.clone();
    // Gradients with no stops and only one stop should automatically be converted by resvg
    // into no fill / plain fill, so there should be at least two stops
    debug_assert!(stops.len() > 1);

    let reference = ctx.deferrer.alloc_ref();

    let mut stitching_function = writer.stitching_function(reference);
    stitching_function.domain(get_default_domain());
    stitching_function.range(get_color_range());
    let mut func_array = stitching_function.insert(Name(b"Functions")).array();
    let mut bounds = vec![];
    let mut encode = vec![];

    // We manually pad the stops if necessary so that they are always in the range from 0-1
    if let Some(first) = stops.first() {
        if first.offset != 0.0 {
            let mut new_stop = first.clone();
            new_stop.offset = StopOffset::new(0.0).unwrap();
            stops.insert(0, new_stop);
        }
    }

    if let Some(last) = stops.last() {
        if last.offset != 1.0 {
            let mut new_stop = last.clone();
            new_stop.offset = StopOffset::new(1.0).unwrap();
            stops.push(new_stop);
        }
    }

    for window in stops.windows(2) {
        let (first, second) = (window[0], window[1]);
        let (first_color, second_color) = (first.color.as_array(), second.color.as_array());
        bounds.push(second.offset.get() as f32);
        let mut exp = ExponentialFunction::start(func_array.push());
        exp.domain(get_default_domain());
        exp.range(get_color_range());
        exp.n(1.0);
        exp.c0(first_color);
        exp.c1(second_color);
        encode.extend(get_default_encode());
    }

    func_array.finish();
    // Remove the last bound since the bounds array only contains the points *in-between*
    // the stops
    bounds.pop();
    stitching_function.bounds(bounds);
    stitching_function.encode(encode);

    reference
}

fn get_default_domain() -> [f32; 2] {
    [0.0, 1.0]
}

fn get_default_encode() -> [f32; 2] {
    [0.0, 1.0]
}

fn get_color_range() -> [f32; 6] {
    [0.0, 1.0, 0.0, 1.0, 0.0, 1.0]
}