use crate::util::context::Context;
use crate::util::helper::{ColorExt, TransformExt};
use pdf_writer::types::ShadingType;
use pdf_writer::writers::ExponentialFunction;
use pdf_writer::{Finish, Name, PdfWriter, Ref, Writer};
use std::rc::Rc;
use usvg::{NormalizedF64, SpreadMethod, StopOffset, Transform, Units};

pub fn create_linear(
    gradient: Rc<usvg::LinearGradient>,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    let (pattern_name, pattern_id) = ctx.deferrer.add_pattern();

    let (x1, x2, y1, y2, mut matrix, gradient) = if gradient.units == Units::ObjectBoundingBox {
        let mut new_gradient = (*gradient).clone();
        gradient.transform.apply_to(&mut new_gradient.x1, &mut new_gradient.y1);
        gradient.transform.apply_to(&mut new_gradient.x2, &mut new_gradient.y2);
        (0.0, 1.0, 0.0, 0.0, Transform::from_bbox(*parent_bbox), Rc::new(new_gradient))
    }   else {
        let mut new_gradient = (*gradient).clone();
        gradient.transform.apply_to(&mut new_gradient.x1, &mut new_gradient.y1);
        gradient.transform.apply_to(&mut new_gradient.x2, &mut new_gradient.y2);
        new_gradient.x1 = new_gradient.x1 / ctx.size.width();
        new_gradient.x2 = new_gradient.x2 / ctx.size.width();
        new_gradient.y1 = new_gradient.y1 / ctx.size.height();
        new_gradient.y2 = new_gradient.y2 / ctx.size.height();
        (0.0, 1.0, 0.0, 0.0,
         Transform::from_bbox(usvg::Rect::new(0.0, 0.0, ctx.size.width(), ctx.size.height()).unwrap()),
        Rc::new(new_gradient))
    };

    let shading_function = get_spread_shading_function(gradient.clone(), writer, ctx);
    let mut shading_pattern = writer.shading_pattern(pattern_id);
    let mut shading = shading_pattern.shading();
    shading.shading_type(ShadingType::Axial);
    shading.color_space().srgb();

    shading.function(shading_function);
    shading.insert(Name(b"Domain")).array().items([0.0, 1.0]);
    shading.extend([true, true]);
    shading.coords([x1 as f32, y2 as f32, x2 as f32, y1 as f32]);
    shading.finish();

    shading_pattern.matrix(matrix.as_array());
    shading_pattern.finish();

    pattern_name
}

fn get_spread_shading_function(
    gradient: Rc<usvg::LinearGradient>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
    let shading_function = get_shading_function(gradient.clone(), writer, ctx);

    // If the x values of the gradient cover the whole span, we don't need to take the spread
    // method into consideration anymore.
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
            let domain: Vec<f32> = Vec::from([0.0, 1.0]);

            if gradient.x1 > 0.0 {
                let pad_function = single_gradient(
                    gradient.stops.first().unwrap().color.as_array(),
                    gradient.stops.first().unwrap().color.as_array(),
                    writer,
                    ctx,
                );
                functions.push(pad_function);
                bounds.push(gradient.x1 as f32);
                encode.extend(get_default_encode());
            }

            functions.push(shading_function);
            bounds.push(gradient.x2 as f32);
            encode.extend(get_default_encode());

            if gradient.x2 < 1.0 {
                let pad_function = single_gradient(
                    gradient.stops.last().unwrap().color.as_array(),
                    gradient.stops.last().unwrap().color.as_array(),
                    writer,
                    ctx,
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
    stitching_function.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
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

    let mut functions = vec![];
    // let mut func_array = stitching_function.insert(Name(b"Functions")).array();
    let mut bounds = vec![];
    let mut encode = vec![];

    // We manually pad the stops if necessary so that they are always in the range from 0-1
    if let Some(first) = stops.first() {
        if first.offset != NormalizedF64::ZERO {
            let mut new_stop = *first;
            new_stop.offset = StopOffset::new(0.0).unwrap();
            stops.insert(0, new_stop);
        }
    }

    if let Some(last) = stops.last() {
        if last.offset != NormalizedF64::ONE {
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
        functions.push(single_gradient(first_color, second_color, writer, ctx));
        encode.extend(get_default_encode());
    }

    // Remove the last bound since the bounds array only contains the points *in-between*
    // the stops
    bounds.pop();

    let mut stitching_function = writer.stitching_function(reference);
    stitching_function.domain([0.0, 1.0]);
    stitching_function.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
    stitching_function.functions(functions);
    stitching_function.bounds(bounds);
    stitching_function.encode(encode);

    reference
}

fn single_gradient(
    c0: impl Into<Vec<f32>>,
    c1: impl Into<Vec<f32>>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
    let reference = ctx.deferrer.alloc_ref();
    let mut exp = writer.exponential_function(reference);

    let c0: Vec<f32> = c0.into();
    let c1: Vec<f32> = c1.into();
    let length = c0.len();

    exp.range([0.0, 1.0].repeat(length));
    exp.c0(c0);
    exp.c1(c1);
    exp.domain([0.0, 1.0]);
    exp.n(1.0);
    exp.finish();
    reference
}

fn get_default_encode() -> [f32; 2] {
    [0.0, 1.0]
}
