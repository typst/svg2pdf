use std::rc::Rc;

use pdf_writer::types::{FunctionShadingType, MaskType};
use pdf_writer::{Content, Filter, Finish, PdfWriter, Ref};
use usvg::{
    LinearGradient, NormalizedF64, Paint, RadialGradient, Stop, StopOffset, Transform,
    Units,
};

use crate::util::context::Context;
use crate::util::helper::{ColorExt, NameExt, RectExt, TransformExt};

/// Turn a group into an shading object (including a soft mask if the gradient contains stop opacities).
/// Returns the name (= the name in the `Resources` dictionary) of the shading object and optionally
/// the name of the soft mask.
pub fn create(
    paint: &Paint,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Option<(Rc<String>, Option<Rc<String>>)> {
    match paint {
        Paint::LinearGradient(l) => {
            Some(create_linear_gradient(l.clone(), parent_bbox, writer, ctx))
        }
        Paint::RadialGradient(r) => {
            Some(create_radial_gradient(r.clone(), parent_bbox, writer, ctx))
        }
        _ => None,
    }
}

struct GradientProperties {
    coords: Vec<f32>,
    shading_type: FunctionShadingType,
    stops: Vec<Stop>,
    transform: Transform,
    units: Units,
}

fn create_linear_gradient(
    gradient: Rc<LinearGradient>,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> (Rc<String>, Option<Rc<String>>) {
    let properties = GradientProperties {
        coords: vec![
            gradient.x1 as f32,
            gradient.y1 as f32,
            gradient.x2 as f32,
            gradient.y2 as f32,
        ],
        shading_type: FunctionShadingType::Axial,
        stops: gradient.stops.clone(),
        transform: gradient.transform,
        units: gradient.units,
    };
    create_shading_pattern(&properties, parent_bbox, writer, ctx)
}

fn create_radial_gradient(
    gradient: Rc<RadialGradient>,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> (Rc<String>, Option<Rc<String>>) {
    let properties = GradientProperties {
        coords: vec![
            gradient.fx as f32,
            gradient.fy as f32,
            0.0,
            gradient.cx as f32,
            gradient.cy as f32,
            gradient.r.get() as f32,
        ],
        shading_type: FunctionShadingType::Radial,
        stops: gradient.stops.clone(),
        transform: gradient.transform,
        units: gradient.units,
    };
    create_shading_pattern(&properties, parent_bbox, writer, ctx)
}

fn create_shading_pattern(
    properties: &GradientProperties,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> (Rc<String>, Option<Rc<String>>) {
    let pattern_ref = ctx.alloc_ref();

    let soft_mask = if properties.stops.iter().any(|stop| stop.opacity.get() < 1.0) {
        Some(get_soft_mask(properties, parent_bbox, writer, ctx))
    } else {
        None
    };

    let mut matrix = if properties.units == Units::ObjectBoundingBox {
        Transform::from_bbox(*parent_bbox)
    } else {
        Transform::default()
    };

    matrix.append(&properties.transform);

    let shading_function_ref =
        get_shading_function(false, &properties.stops, writer, ctx);
    let mut shading_pattern = writer.shading_pattern(pattern_ref);
    let mut shading = shading_pattern.function_shading();
    shading.shading_type(properties.shading_type);
    shading.color_space().srgb();

    shading.function(shading_function_ref);
    shading.coords(properties.coords.iter().copied());
    shading.extend([true, true]);
    shading.finish();

    shading_pattern.matrix(matrix.as_array());
    shading_pattern.finish();

    (ctx.deferrer.add_pattern(pattern_ref), soft_mask)
}

fn get_soft_mask(
    properties: &GradientProperties,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Rc<String> {
    ctx.deferrer.push();
    let x_object_id = ctx.alloc_ref();
    let shading_ref = ctx.alloc_ref();
    let shading_name = ctx.deferrer.add_shading(shading_ref);
    let bbox = ctx.get_rect().as_pdf_rect();

    let mut transform = properties.transform;
    transform.append(&if properties.units == Units::ObjectBoundingBox {
        Transform::from_bbox(*parent_bbox)
    } else {
        Transform::default()
    });

    let shading_function_ref = get_shading_function(true, &properties.stops, writer, ctx);
    let mut shading = writer.function_shading(shading_ref);
    shading.shading_type(properties.shading_type);
    shading.color_space().d65_gray();

    shading.function(shading_function_ref);
    shading.coords(properties.coords.iter().copied());
    shading.extend([true, true]);
    shading.finish();

    let mut content = Content::new();
    content.transform(transform.as_array());
    content.shading(shading_name.as_name());
    let content_stream = ctx.finish_content(content);

    let mut x_object = writer.form_xobject(x_object_id, &content_stream);
    ctx.deferrer.pop(&mut x_object.resources());
    x_object
        .group()
        .transparency()
        .isolated(true)
        .knockout(false)
        .color_space()
        .d65_gray();

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    x_object.bbox(bbox);
    x_object.finish();

    let gs_ref = ctx.alloc_ref();
    let mut gs = writer.ext_graphics(gs_ref);
    gs.soft_mask()
        .subtype(MaskType::Luminosity)
        .group(x_object_id)
        .finish();

    ctx.deferrer.add_graphics_state(gs_ref)
}

fn get_shading_function(
    alpha: bool,
    stops: &[Stop],
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
    let reference = ctx.alloc_ref();
    let mut stops = stops.to_owned();
    // Gradients with no stops and only one stop should automatically be converted by resvg
    // into no fill / plain fill, so there should be at least two stops
    debug_assert!(stops.len() > 1);

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
        let (first_color, second_color) = if alpha {
            (vec![first.opacity.get() as f32], vec![second.opacity.get() as f32])
        } else {
            (Vec::from(first.color.as_array()), Vec::from(second.color.as_array()))
        };

        bounds.push(second.offset.get() as f32);
        functions.push(single_gradient(first_color, second_color, writer, ctx));
        encode.extend([0.0, 1.0]);
    }

    // Remove the last bound since the bounds array only contains the points *in-between*
    // the stops
    bounds.pop();

    let mut stitching_function = writer.stitching_function(reference);
    stitching_function.domain([0.0, 1.0]);
    if alpha {
        stitching_function.range([0.0, 1.0]);
    } else {
        stitching_function.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
    }
    stitching_function.functions(functions);
    stitching_function.bounds(bounds);
    stitching_function.encode(encode);
    reference
}

fn single_gradient(
    c0: Vec<f32>,
    c1: Vec<f32>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
    assert_eq!(c0.len(), c1.len());

    let reference = ctx.alloc_ref();
    let mut exp = writer.exponential_function(reference);
    let length = c0.len();

    exp.range([0.0, 1.0].repeat(length));
    exp.c0(c0);
    exp.c1(c1);
    exp.domain([0.0, 1.0]);
    exp.n(1.0);
    exp.finish();
    reference
}
