use crate::util::context::Context;
use crate::util::helper::{ColorExt, TransformExt};
use nalgebra::{Point2, Vector2};
use pdf_writer::types::ShadingType;
use pdf_writer::writers::ExponentialFunction;
use pdf_writer::{Finish, Name, PdfWriter, Ref, Writer};
use std::rc::Rc;
use usvg::{
    LinearGradient, NormalizedF64, Size, SpreadMethod, StopOffset, Transform, Units,
};

pub fn create_linear(
    gradient: Rc<usvg::LinearGradient>,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    let (pattern_name, pattern_id) = ctx.deferrer.add_pattern();
    let gradient_transform = gradient.transform;

    let bounding_rect = usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap();

    let mut gradient = (*gradient).clone();
    apply_gradient_transform(&gradient_transform, &mut gradient);

    let matrix = if gradient.units == Units::ObjectBoundingBox {
        Transform::from_bbox(*parent_bbox)
    } else {
        normalize_gradient(&ctx.size, &mut gradient);
        Transform::from_bbox(usvg::Rect::new(0.0, 0.0, ctx.size.width(), ctx.size.height()).unwrap())
    };


    let (c1, c2) = get_coordinate_points(
        Point2::from([gradient.x1, gradient.y1]),
        Point2::from([gradient.x2, gradient.y2]),
        Point2::from([bounding_rect.x(), bounding_rect.y()]),
        Point2::from([
            bounding_rect.x() + bounding_rect.width(),
            bounding_rect.y() + bounding_rect.width(),
        ]),
    );
    let gradient = Rc::new(gradient);

    let shading_function =
        get_spread_shading_function((c1.x, c2.x), gradient.clone(), writer, ctx);
    let mut shading_pattern = writer.shading_pattern(pattern_id);
    let mut shading = shading_pattern.shading();
    shading.shading_type(ShadingType::Axial);
    shading.color_space().srgb();

    shading.function(shading_function);
    shading
        .insert(Name(b"Domain"))
        .array()
        .items([c1.x as f32, c2.x as f32]);
    shading.coords([c1.x as f32, c1.y as f32, c2.x as f32, c2.y as f32]);
    shading.extend([true, true]);
    shading.finish();

    shading_pattern.matrix(matrix.as_array());
    shading_pattern.finish();

    pattern_name
}

fn get_coordinate_points(
    line_p1: Point2<f64>,
    line_p2: Point2<f64>,
    rect_p1: Point2<f64>,
    rect_p2: Point2<f64>,
) -> (Point2<f64>, Point2<f64>) {
    let line_vertices = [line_p1, line_p2];

    let rect_vertices = [
        Point2::from([rect_p1.x, rect_p1.y]),
        Point2::from([rect_p1.x, rect_p2.y]),
        Point2::from([rect_p2.x, rect_p1.y]),
        Point2::from([rect_p2.x, rect_p2.y]),
    ];

    let line_vector = &line_vertices[1] - &line_vertices[0];

    let mut a_min = f64::MAX;
    let mut a_max = f64::MIN;

    for rect_point in rect_vertices {
        let q = rect_point - line_vertices[0];
        let a = 1.0 / (line_vector.x * line_vector.x + line_vector.y * line_vector.y)
            * (line_vector.x * q.x + line_vector.y * q.y);
        a_min = a_min.min(a);
        a_max = a_max.max(a);
    }

    let new_line_point_1 = line_vertices[0] + line_vector * a_min;
    let new_line_point_2 = line_vertices[0] + line_vector * a_max;
    (new_line_point_1, new_line_point_2)
}

fn apply_gradient_transform(transform: &Transform, gradient: &mut LinearGradient) {
    transform.apply_to(&mut gradient.x1, &mut gradient.y1);
    transform.apply_to(&mut gradient.x2, &mut gradient.y2);
}

fn normalize_gradient(size: &Size, gradient: &mut LinearGradient) {
    gradient.x1 = gradient.x1 / size.width();
    gradient.x2 = gradient.x2 / size.width();
    gradient.y1 = gradient.y1 / size.height();
    gradient.y2 = gradient.y2 / size.height();
}

fn get_spread_shading_function(
    (x1, x2): (f64, f64),
    gradient: Rc<usvg::LinearGradient>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
    let single_shading_function =
        get_single_shading_function(gradient.clone(), writer, ctx);
    let spread_shading_function = ctx.deferrer.alloc_ref();

    let (bound_min, bound_max, x_min, x_max) =
        (
            x1.min(x2),
            x1.max(x2),
            gradient.x1.min(gradient.x2),
            gradient.x1.max(gradient.x2)
        );

    let generate_repeating_pattern = |reflect: bool| {
        let (sequences, domain) = {
            let reflect_cycle = if reflect { [true, false] } else { [false, false] }
                .into_iter()
                .cycle();

            let mut backward_reflect_cycle = reflect_cycle.clone();
            let x_delta = (gradient.x2 - gradient.x1).abs();
            let mut sub_ranges: Vec<(f32, f32, bool)> =
                vec![(gradient.x1 as f32, gradient.x2 as f32, false)];

            let (mut x_min, mut x_max) = (x_min, x_max);
            while x_min > bound_min {
                x_min -= x_delta;
                sub_ranges.push((
                    x_min as f32,
                    (x_min + x_delta) as f32,
                    backward_reflect_cycle.next().unwrap(),
                ));
            }

            sub_ranges.reverse();

            let mut forward_reflect_cycle = reflect_cycle;
            while x_max < bound_max {
                sub_ranges.push((
                    x_max as f32,
                    (x_max + x_delta) as f32,
                    forward_reflect_cycle.next().unwrap(),
                ));
                x_max += x_delta;
            }

            (sub_ranges, vec![x_min as f32, x_max as f32])
        };
        let mut bounds: Vec<f32> = vec![];
        let mut functions = vec![];
        let mut encode: Vec<f32> = vec![];

        for sequence in sequences {
            bounds.push(sequence.1);
            functions.push(single_shading_function);
            encode.extend(if sequence.2 { [1.0, 0.0] } else { [0.0, 1.0] });
        }

        bounds.pop();

        (functions, bounds, domain, encode)
    };

    let (functions, bounds, domain, encode) = match gradient.spread_method {
        SpreadMethod::Pad => {
            let mut functions = vec![];
            let mut bounds: Vec<f32> = vec![];
            let mut encode = vec![];
            let domain: Vec<f32> = Vec::from([bound_min as f32, bound_max as f32]);

            if x_min > bound_min {
                let pad_function = single_gradient(
                    gradient.stops.first().unwrap().color.as_array(),
                    gradient.stops.first().unwrap().color.as_array(),
                    writer,
                    ctx,
                );
                functions.push(pad_function);
                bounds.push(x_min as f32);
                encode.extend([0.0, 1.0]);
            }

            functions.push(single_shading_function);
            bounds.push(x_max as f32);
            encode.extend([0.0, 1.0]);

            if x_max < bound_max {
                let pad_function = single_gradient(
                    gradient.stops.last().unwrap().color.as_array(),
                    gradient.stops.last().unwrap().color.as_array(),
                    writer,
                    ctx,
                );
                functions.push(pad_function);
                bounds.push(bound_max as f32);
                encode.extend([0.0, 1.0]);
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

fn get_single_shading_function(
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
