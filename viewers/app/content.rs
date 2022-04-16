use crate::app::{App, FieldType};
use anyhow::Result;
use engine::state::{BranchState, LeafState};

impl App {
    pub(crate) fn draw_content(&mut self, ui: &mut egui::Ui) -> Result<()> {
        use itertools::Itertools;

        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        self.handle_input(response);

        let max_rect = ui.max_rect();
        let (x1, y1) = self.pan.to_model_fu(max_rect.min.into());
        let (x2, y2) = self.pan.to_model_fu(max_rect.max.into());

        let bounding_box = quadtree::Rect::corners(x1, y1, x2, y2);

        let mut qtree_visitor = DrawQtreeVisitor::new(self, &painter);
        self.engine
            .qtree
            .visit_rect(&mut qtree_visitor, &bounding_box)?;

        self.diagnostics.tiles = qtree_visitor.visited;

        // 5 pixel resolution
        let spline_scale = f64::max(
            self.options.spline_resolution as f64 / self.pan.scale as f64,
            0.2,
        );

        self.diagnostics.metro_vertices = 0;
        self.diagnostics.highway_vertices = 0;

        // TODO: don't sort every iteration!!
        for (id, metro_line) in self.engine.metro_lines.iter().sorted() {
            let mut spline_visitor = DrawSplineVisitor::new(self, &painter);
            metro_line.visit_spline(&mut spline_visitor, spline_scale, &bounding_box)?;
            self.diagnostics.metro_vertices += spline_visitor.visited;
        }

        for (id, highway_segment) in self.engine.highways.get_segments().iter().sorted() {
            let mut spline_visitor = DrawSplineVisitor::new(self, &painter);
            highway_segment.visit_spline(&mut spline_visitor, spline_scale, &bounding_box)?;
            self.diagnostics.highway_vertices += spline_visitor.visited;
        }

        for (id, highway_junction) in self.engine.highways.get_junctions().iter().sorted() {
            if let Some(_) = highway_junction.ramp {
                let (x, y) = highway_junction.location;
                let pos = egui::Pos2::from(self.pan.to_screen_ff((x as f32, y as f32)));
                painter.circle(
                    pos,
                    2.0,
                    egui::Color32::from_gray(255),
                    egui::Stroke::none(),
                );
            }
        }

        let route_input = route::SplineConstructionInput {
            metro_lines: &self.engine.metro_lines,
            highways: &self.engine.highways,
            state: &route::WorldState::new(),
            tile_size: self.engine.config.min_tile_size as f64,
        };

        // draw route from the query interface
        for route in &self.route_query.current_routes {
            let mut route_visitor = DrawSplineVisitor::new(self, &painter);
            route.visit_spline(
                &mut route_visitor,
                spline_scale,
                &bounding_box,
                &route_input,
            )?;
            if let Some(key) =
                route.sample_engine_time(self.engine.time_state.current_time as f64, &route_input)
            {
                let (x, y) = key.position;
                let pos = egui::Pos2::from(self.pan.to_screen_ff((x as f32, y as f32)));
                painter.circle(
                    pos,
                    6.0,
                    egui::Color32::from_rgb(0, 0, 255),
                    egui::Stroke::none(),
                );
            }
        }

        for agent in self.engine.agents.values() {
            if let agent::AgentState::Route(route) = &agent.state {
                if let Some(key) = route
                    .sample_engine_time(self.engine.time_state.current_time as f64, &route_input)
                {
                    let (x, y) = key.position;
                    let pos = egui::Pos2::from(self.pan.to_screen_ff((x as f32, y as f32)));
                    painter.circle(
                        pos,
                        5.0,
                        egui::Color32::from_rgb(0, 0, 255),
                        egui::Stroke::none(),
                    );
                }
            }
        }

        Ok(())
    }

    fn update_scale(&mut self, scale: f32, mx: f32, my: f32) {
        let new_scale = f32::max(f32::min(scale, self.pan.max_scale), self.pan.min_scale);

        // zoom centered on mouse
        self.pan.tx =
            (mx * self.pan.scale - mx * new_scale + self.pan.tx * new_scale) / self.pan.scale;
        self.pan.ty =
            (my * self.pan.scale - my * new_scale + self.pan.ty * new_scale) / self.pan.scale;

        self.pan.scale = new_scale;
    }

    fn handle_input(&mut self, response: egui::Response) {
        let scroll_delta = -response.ctx.input().scroll_delta.y;
        if scroll_delta != 0.0 {
            // desktop

            let scale = self.pan.scale * 1.1_f32.powf(-scroll_delta / 10.0);
            if let Some(pos) = { response.ctx.input().pointer.interact_pos() } {
                self.update_scale(scale, pos.x, pos.y);
            };
        }

        if let Some(multitouch) = { response.ctx.multi_touch() } {
            // mobile

            let delta = multitouch.translation_delta;
            self.pan.tx += delta.x;
            self.pan.ty += delta.y;

            let gesture_center = multitouch.average_pos;

            let scale = self.pan.scale * multitouch.zoom_delta;
            self.update_scale(scale, gesture_center.x, gesture_center.y);
        } else {
            if response.dragged() {
                // desktop

                // NOTE: Would also apply on mobile since we generate fake pointer events,
                // but we prefer to use the multitouch measurement since it accounts for
                // all active touches, not just one.

                let delta = response.drag_delta();
                self.pan.tx += delta.x;
                self.pan.ty += delta.y;
            }
        }
    }
}

struct DrawQtreeVisitor<'a, 'b> {
    app: &'a App,
    painter: &'b egui::Painter,
    visited: u64,
}

impl<'a, 'b> DrawQtreeVisitor<'a, 'b> {
    fn new(app: &'a App, painter: &'b egui::Painter) -> Self {
        Self {
            app,
            painter,
            visited: 0,
        }
    }

    fn get_rect(&self, data: &quadtree::VisitData) -> egui::Rect {
        let width = data.width as f32 * self.app.pan.scale;
        let origin = egui::Pos2::from(self.app.pan.to_screen_uf((data.x, data.y)));
        egui::Rect::from_two_pos(origin, (origin.x + width, origin.y + width).into())
    }

    fn get_full_rect(&self, data: &quadtree::VisitData) -> egui::Rect {
        // TODO: this doesn't seem to be working? so using the +0.5 for now
        let origin = egui::Pos2::from(self.app.pan.to_screen_uf((data.x, data.y)));
        let corner = egui::Pos2::from(
            self.app
                .pan
                .to_screen_uf((data.x + data.width, data.y + data.width)),
        );
        let extra = egui::Vec2::new(0.5, 0.5);
        egui::Rect::from_two_pos(
            self.painter.round_pos_to_pixels(origin - extra),
            self.painter.round_pos_to_pixels(corner + extra),
        )
    }

    fn maybe_draw_field(
        &mut self,
        fields: &fields::FieldsState,
        data: &quadtree::VisitData,
        is_leaf: bool,
    ) {
        let width = data.width as f32 * self.app.pan.scale;
        let threshold = self.app.options.field_resolution as f32;
        if is_leaf || (width >= threshold && width < threshold * 2.0) {
            if let Some(field) = self.app.field {
                let hue = match field {
                    FieldType::Population => {
                        let peak = 0.15;
                        f32::min(fields.population.density as f32, peak) / peak * 0.3
                    }
                    FieldType::Employment => {
                        let peak = 0.3;
                        f32::min(fields.employment.density as f32, peak) / peak * 0.6
                    }
                    FieldType::LandValue => 0.0,
                };
                let color = egui::color::Hsva::new(hue, 0.8, 0.8, 0.5);
                let rect = self.get_full_rect(data);
                self.painter
                    .rect_filled(rect, egui::Rounding::none(), color);
            }
        }
    }
}

impl<'a, 'b> quadtree::Visitor<BranchState, LeafState, anyhow::Error> for DrawQtreeVisitor<'a, 'b> {
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState,
        data: &quadtree::VisitData,
    ) -> Result<bool> {
        let should_descend =
            data.width as f32 * self.app.pan.scale >= self.app.options.min_tile_size as f32;

        if !should_descend && self.app.field.is_none() {
            let full_rect = self.get_full_rect(data);
            self.painter.rect_filled(
                full_rect,
                egui::Rounding::none(),
                egui::Color32::from_gray(100),
            );
            self.visited += 1;
        }

        Ok(should_descend)
    }

    fn visit_leaf(&mut self, leaf: &LeafState, data: &quadtree::VisitData) -> Result<()> {
        let width = data.width as f32 * self.app.pan.scale;
        let rect = self.get_rect(data);
        let full_rect = self.get_full_rect(data);

        use tiles::Tile::*;
        match &leaf.tile {
            WaterTile(tiles::WaterTile {}) => {
                self.painter.rect_filled(
                    full_rect,
                    egui::Rounding::none(),
                    egui::Color32::from_rgb(0, 0, 150),
                );
            }
            HousingTile(tiles::HousingTile { density, .. }) => {
                self.painter.circle_filled(
                    rect.center(),
                    width / 8.0,
                    egui::Color32::from_gray(255),
                );
            }
            WorkplaceTile(tiles::WorkplaceTile { density, .. }) => {
                self.painter.add(regular_poly::<3>(
                    self.painter,
                    rect.center().into(),
                    width / 6.0,
                    -std::f32::consts::FRAC_PI_2,
                    egui::Color32::from_gray(255),
                    egui::Stroke::none(),
                ));
            }
            MetroStationTile(tiles::MetroStationTile { x, y, ids, .. }) => {
                self.painter.circle_stroke(
                    rect.center(),
                    width / 4.0,
                    (1.0, egui::Color32::from_gray(255)),
                );
            }
            _ => (),
        }
        self.visited += 1;

        self.maybe_draw_field(&leaf.fields, data, true);

        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &BranchState,
        data: &quadtree::VisitData,
    ) -> Result<()> {
        self.maybe_draw_field(&branch.fields, data, false);
        Ok(())
    }
}

fn regular_poly<const N: usize>(
    painter: &egui::Painter,
    (x, y): (f32, f32),
    radius: f32,
    theta: f32,
    fill: egui::Color32,
    stroke: egui::Stroke,
) -> egui::Shape {
    use std::f32::consts::PI;
    let mut points = Vec::with_capacity(N);

    for i in 0..N {
        let t = (PI * 2.0) / N as f32 * i as f32 + theta;
        points.push((x + t.cos() * radius, y + t.sin() * radius).into());
    }

    egui::Shape::Path(egui::epaint::PathShape {
        points,
        closed: true,
        fill,
        stroke,
    })
}

struct DrawSplineVisitor<'a, 'b> {
    app: &'a App,
    painter: &'b egui::Painter,

    last_point: Option<(f32, f32)>,
    visited: u64,
}

impl<'a, 'b> DrawSplineVisitor<'a, 'b> {
    fn new(app: &'a App, painter: &'b egui::Painter) -> Self {
        Self {
            app,
            painter,
            last_point: None,
            visited: 0,
        }
    }

    fn visit(
        &mut self,
        color: &egui::Color32,
        line_width: f32,
        vertex: cgmath::Vector2<f64>,
        t: f64,
    ) -> Result<()> {
        let point = self
            .app
            .pan
            .to_screen_ff((vertex.x as f32, vertex.y as f32));

        if let Some(last_point) = self.last_point {
            self.painter
                .line_segment([last_point.into(), point.into()], (line_width, *color));
        }
        self.visited += 1;
        self.last_point = Some(point);

        Ok(())
    }
}

impl<'a, 'b> metro::SplineVisitor<metro::MetroLine, cgmath::Vector2<f64>, anyhow::Error>
    for DrawSplineVisitor<'a, 'b>
{
    fn visit(
        &mut self,
        line: &metro::MetroLine,
        vertex: cgmath::Vector2<f64>,
        t: f64,
    ) -> Result<()> {
        let color = egui::Color32::from_rgb(line.color.red, line.color.green, line.color.blue);
        self.visit(&color, 2.0, vertex, t)
    }
}

impl<'a, 'b> highway::SplineVisitor<highway::HighwaySegment, cgmath::Vector2<f64>, anyhow::Error>
    for DrawSplineVisitor<'a, 'b>
{
    fn visit(
        &mut self,
        segment: &highway::HighwaySegment,
        vertex: cgmath::Vector2<f64>,
        t: f64,
    ) -> Result<()> {
        self.visit(&egui::Color32::from_gray(204), 1.0, vertex, t)
    }
}

impl<'a, 'b> route::SplineVisitor<route::Route, route::RouteKey, anyhow::Error>
    for DrawSplineVisitor<'a, 'b>
{
    fn visit(&mut self, route: &route::Route, key: route::RouteKey, t: f64) -> Result<()> {
        self.visit(
            &egui::Color32::from_rgb(0, 0, 255),
            5.0,
            key.position.into(),
            t,
        )
    }
}
