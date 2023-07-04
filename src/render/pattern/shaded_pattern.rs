use crate::util::context::Context;
use crate::util::helper::{ColorExt, RectExt};
use pdf_writer::types::ShadingType;
use pdf_writer::writers::ExponentialFunction;
use pdf_writer::{Finish, Name, PdfWriter, Ref, Writer};
use std::rc::Rc;
use usvg::{SpreadMethod, StopOffset};

pub fn create_linear(
    gradient: Rc<usvg::LinearGradient>,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    let (pattern_name, pattern_id) = ctx.deferrer.add_pattern();
    let shading_function = get_spread_shading_function(gradient, writer, ctx);
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
    let shading_function = get_shading_function(gradient.clone(), writer, ctx);

    if gradient.x1 == 0.0 && gradient.x2 == 1.0 {
        return shading_function;
    }

    let spread_shading_function = ctx.deferrer.alloc_ref();

    let generate_repeating_pattern = |reflect: bool| {
        let (sequences, min, max) = {
            let reflect_cycle = if reflect { [true, false] } else { [false, false] }
                .into_iter()
                .cycle();

            let mut backward_reflect_cycle = reflect_cycle.clone();
            let x_delta = (gradient.x2 - gradient.x1) as f32;
            let mut sub_ranges: Vec<(f32, f32, bool)> =
                vec![(gradient.x1 as f32, gradient.x2 as f32, false)];

            let mut min = gradient.x1 as f32;
            while min > 0.0 {
                min -= x_delta;
                sub_ranges.push((
                    min,
                    min + x_delta,
                    backward_reflect_cycle.next().unwrap(),
                ));
            }

            sub_ranges.reverse();

            let mut forward_reflect_cycle = reflect_cycle;
            let mut max = gradient.x2 as f32;
            while max < 1.0 {
                sub_ranges.push((
                    max,
                    max + x_delta,
                    forward_reflect_cycle.next().unwrap(),
                ));
                max += x_delta;
            }

            (sub_ranges, min, max)
        };
        let mut bounds: Vec<f32> = vec![];
        let mut functions = vec![];
        let mut encode: Vec<f32> = vec![];

        for sequence in sequences {
            bounds.push(sequence.1);
            functions.push(shading_function);
            encode.extend(if sequence.2 { [1.0, 0.0] } else { get_default_encode() });
        }

        bounds.pop();

        (functions, bounds, vec![min, max], encode)
    };

    let (functions, bounds, domain, encode) = match gradient.spread_method {
        SpreadMethod::Pad => {
            let mut functions = vec![];
            let mut bounds: Vec<f32> = vec![];
            let mut encode = vec![];
            let domain: Vec<f32> = Vec::from(get_default_domain());

            if gradient.x1 > 0.0 {
                let pad_ref = ctx.deferrer.alloc_ref();
                let pad_function =
                    single_color_function(gradient.stops[0].color, writer, pad_ref);
                functions.push(pad_function);
                bounds.push(gradient.x1 as f32);
                encode.extend(get_default_encode());
            }

            functions.push(shading_function);
            bounds.push(gradient.x2 as f32);
            encode.extend(get_default_encode());

            if gradient.x2 < 1.0 {
                let pad_ref = ctx.deferrer.alloc_ref();
                let pad_function = single_color_function(
                    gradient.stops.last().unwrap().color,
                    writer,
                    pad_ref,
                );
                functions.push(pad_function);
                bounds.push(1.0);
                encode.extend(get_default_encode());
            }

            bounds.pop();

            (functions, bounds, domain, encode)
        }
        SpreadMethod::Reflect => generate_repeating_pattern(true),
        SpreadMethod::Repeat => generate_repeating_pattern(false),
    };

    let mut stitching_function = writer.stitching_function(spread_shading_function);
    stitching_function.range(get_color_range());
    stitching_function.functions(functions);
    stitching_function.bounds(bounds);
    stitching_function.domain(domain);
    stitching_function.encode(encode);

    spread_shading_function
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
            let mut new_stop = *first;
            new_stop.offset = StopOffset::new(0.0).unwrap();
            stops.insert(0, new_stop);
        }
    }

    if let Some(last) = stops.last() {
        if last.offset != 1.0 {
            let mut new_stop = *last;
            new_stop.offset = StopOffset::new(1.0).unwrap();
            stops.push(new_stop);
        }
    }

    for window in stops.windows(2) {
        let (first, second) = (window[0], window[1]);
        let (first_color, second_color) =
            (first.color.as_array(), second.color.as_array());
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

fn single_color_function(
    color: usvg::Color,
    writer: &mut PdfWriter,
    reference: Ref,
) -> Ref {
    let mut exp = writer.exponential_function(reference);

    exp.c0(color.as_array());
    exp.c1(color.as_array());
    exp.domain(get_default_domain());
    exp.range(get_color_range());
    exp.n(1.0);
    exp.finish();
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
