use crate::app::{App, FieldType};
use anyhow::Result;
use state::{BranchState, LeafState};

impl App {
    pub(crate) fn get_bounding_box(&self, ui: &egui::Ui) -> quadtree::Rect {
        let max_rect = ui.clip_rect();
        let (x1, y1) = self.pan.to_model_fu(max_rect.min.into());
        let (x2, y2) = self.pan.to_model_fu(max_rect.max.into());

        quadtree::Rect::corners(x1, y1, x2, y2)
    }

    pub(crate) fn draw_content(&mut self, ui: &mut egui::Ui) -> Result<()> {
        use itertools::Itertools;

        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        self.handle_input(response);

        let bounding_box = self.get_bounding_box(ui);

        let mut qtree_visitor = DrawQtreeVisitor::new(self, &painter);
        self.engine
            .state
            .qtree
            .visit_rect(&mut qtree_visitor, &bounding_box)?;

        self.diagnostics.tiles = qtree_visitor.visited;

        // 5 pixel resolution
        let spline_scale = f64::max(
            self.options.spline_resolution as f64 / self.pan.scale as f64,
            0.2,
        );

        let traffic = self.overlay.traffic.then(|| &self.engine.world_state);

        self.diagnostics.metro_vertices = 0;
        self.diagnostics.highway_vertices = 0;
        self.diagnostics.agents = 0;

        for (id, highway_segment) in self.engine.state.highways.get_segments() {
            if bounding_box.intersects(&highway_segment.bounds) {
                let mut spline_visitor = DrawSplineVisitor::new(self, &painter, traffic);
                highway_segment.visit_spline(&mut spline_visitor, spline_scale, &bounding_box)?;
                self.diagnostics.highway_vertices += spline_visitor.visited;
            }
        }

        if self.pan.scale >= 4.0 {
            for (id, highway_junction) in self.engine.state.highways.get_junctions() {
                if let Some(_) = highway_junction.ramp {
                    let (x, y) = highway_junction.location;
                    if bounding_box.contains(x as u64, y as u64) {
                        let pos = egui::Pos2::from(self.pan.to_screen_ff((x as f32, y as f32)));
                        painter.circle(
                            pos,
                            self.scale_point(4.0, 2.0),
                            egui::Color32::from_gray(255),
                            egui::Stroke::none(),
                        );
                    }
                }
            }
        }

        for (id, metro_line) in &self.engine.state.metro_lines {
            if bounding_box.intersects(&metro_line.get_bounds()) {
                let mut spline_visitor = DrawSplineVisitor::new(self, &painter, traffic);
                metro_line.visit_spline(&mut spline_visitor, spline_scale, &bounding_box)?;
                self.diagnostics.metro_vertices += spline_visitor.visited;
            }
        }

        // draw route from the query interface
        for route in &self.route_query.current_routes {
            if bounding_box.intersects(&route.bounds) {
                let mut route_visitor = DrawSplineVisitor::new(self, &painter, traffic);
                route.visit_spline(
                    &mut route_visitor,
                    spline_scale,
                    &bounding_box,
                    &self.engine.state,
                )?;
            }
        }

        // only render routes if the simulation is slow enough to see them and we are zoomed in
        // sufficiently far
        if self.engine.time_state.should_render_motion() && self.pan.scale >= 2.0 {
            for agent in self.engine.agents.values() {
                if let agent::AgentState::Route(route_state) = &agent.state {
                    // NOTE: this draws a lot more than needed, but it also avoids computing the
                    // time spline for each route unless necessary
                    if bounding_box.intersects(&route_state.route.bounds) {
                        if let Some(key) = route_state
                            .sample(self.engine.time_state.current_time, &self.engine.state)
                        {
                            let (x, y) = key.position;
                            let pos = egui::Pos2::from(self.pan.to_screen_ff((x, y)));
                            painter.circle(
                                pos,
                                self.scale_point(2.0, 5.0),
                                egui::Color32::from_rgb(0, 0, 255),
                                egui::Stroke::none(),
                            );
                            self.diagnostics.agents += 1;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn scale_point(&self, scale_cutoff: f32, max_size: f32) -> f32 {
        max_size.min(self.pan.scale / scale_cutoff)
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
        fields: &engine::FieldsState,
        data: &quadtree::VisitData,
        is_leaf: bool,
    ) {
        let width = data.width as f32 * self.app.pan.scale;
        let threshold = self.app.options.field_resolution as f32;
        if is_leaf || (width >= threshold && width < threshold * 2.0) {
            if let Some(field) = self.app.overlay.field {
                let hue = field.hue(fields, data);
                let color = egui::color::Hsva::new(hue, 0.8, 0.8, 0.5);
                let rect = self.get_full_rect(data);
                self.painter
                    .rect_filled(rect, egui::Rounding::none(), color);
            }
        }
    }
}

impl<'a, 'b>
    quadtree::Visitor<
        BranchState<engine::FieldsState>,
        LeafState<engine::FieldsState>,
        anyhow::Error,
    > for DrawQtreeVisitor<'a, 'b>
{
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState<engine::FieldsState>,
        data: &quadtree::VisitData,
    ) -> Result<bool> {
        let should_descend =
            data.width as f32 * self.app.pan.scale >= self.app.options.min_tile_size as f32;

        if !should_descend && self.app.overlay.field.is_none() {
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

    fn visit_leaf(
        &mut self,
        leaf: &LeafState<engine::FieldsState>,
        data: &quadtree::VisitData,
    ) -> Result<()> {
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
        branch: &BranchState<engine::FieldsState>,
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

struct DrawSplineVisitor<'a, 'b, 'c> {
    app: &'a App,
    painter: &'b egui::Painter,

    traffic: Option<&'c route::WorldStateImpl>,

    visited: u64,
}

impl<'a, 'b, 'c> DrawSplineVisitor<'a, 'b, 'c> {
    fn new(
        app: &'a App,
        painter: &'b egui::Painter,
        traffic: Option<&'c route::WorldStateImpl>,
    ) -> Self {
        Self {
            app,
            painter,
            traffic,
            visited: 0,
        }
    }

    fn visit(
        &mut self,
        color: &egui::Color32,
        line_width: f32,
        traffic_factor: Option<f64>,
        vertex: cgmath::Vector2<f64>,
        t: f64,
        prev: Option<cgmath::Vector2<f64>>,
    ) -> Result<()> {
        let point = self
            .app
            .pan
            .to_screen_ff((vertex.x as f32, vertex.y as f32));

        let (color, line_width) = match traffic_factor {
            Some(traffic_factor) => {
                let scaled = (traffic_factor - 1.0).min(5.0) / 5.0;
                let hue = 1.0 / 3.0 - (scaled * 1.0 / 3.0);
                let color = egui::color::Hsva::new(hue as f32, 1.0, 1.0, 1.0).into();
                let line_width_factor = 2.0 + 2.0 * scaled as f32;
                (color, line_width * line_width_factor)
            }
            None => (*color, line_width),
        };

        if let Some(prev) = prev {
            let prev_point = self.app.pan.to_screen_ff((prev.x as f32, prev.y as f32));
            self.painter
                .line_segment([prev_point.into(), point.into()], (line_width, color));
        }
        self.visited += 1;

        Ok(())
    }
}

impl<'a, 'b, 'c> metro::SplineVisitor<metro::MetroLine, cgmath::Vector2<f64>, anyhow::Error>
    for DrawSplineVisitor<'a, 'b, 'c>
{
    fn visit(
        &mut self,
        line: &metro::MetroLine,
        vertex: cgmath::Vector2<f64>,
        t: f64,
        prev: Option<cgmath::Vector2<f64>>,
    ) -> Result<()> {
        let color = match self.traffic {
            None => egui::Color32::from_rgb(line.color.red, line.color.green, line.color.blue),
            Some(_) => {
                // when we're drawing traffic, make all metro lines white so that we can distinguish
                // traffic colors from metro line colors
                egui::Color32::from_gray(255)
            }
        };
        self.visit(&color, 2.0, None, vertex, t, prev)
    }
}

impl<'a, 'b, 'c>
    highway::SplineVisitor<highway::HighwaySegment, cgmath::Vector2<f64>, anyhow::Error>
    for DrawSplineVisitor<'a, 'b, 'c>
{
    fn visit(
        &mut self,
        segment: &highway::HighwaySegment,
        vertex: cgmath::Vector2<f64>,
        t: f64,
        prev: Option<cgmath::Vector2<f64>>,
    ) -> Result<()> {
        use route::WorldState;
        self.visit(
            &egui::Color32::from_gray(204),
            1.0,
            self.traffic.map(|t| {
                segment.congested_travel_factor(
                    self.app.engine.state.config.min_tile_size,
                    self.app.engine.state.config.people_per_sim,
                    t.get_highway_segment_travelers(segment.id) as u32,
                )
            }),
            vertex,
            t,
            prev,
        )
    }
}

impl<'a, 'b, 'c> route::SplineVisitor<route::Route, route::RouteKey, anyhow::Error>
    for DrawSplineVisitor<'a, 'b, 'c>
{
    fn visit(
        &mut self,
        route: &route::Route,
        key: route::RouteKey,
        t: f64,
        prev: Option<route::RouteKey>,
    ) -> Result<()> {
        let (x, y) = key.position.into();
        self.visit(
            &egui::Color32::from_rgb(0, 0, 255),
            5.0,
            None,
            (x as f64, y as f64).into(),
            t,
            prev.map(|prev| {
                let (x, y) = prev.position.into();
                (x as f64, y as f64).into()
            }),
        )
    }
}
