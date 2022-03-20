use crate::App;
use anyhow::Result;
use engine::state::{BranchState, LeafState};

impl App {
    pub(crate) fn draw_content(&mut self, ui: &mut egui::Ui) -> Result<()> {
        let size = ui.available_size();
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        self.handle_input(response);

        let (x1, y1) = self.pan.to_model_fu((0.0, 0.0));
        let (x2, y2) = self.pan.to_model_fu((size.x.into(), size.y.into()));

        let bounding_box = quadtree::Rect::corners(x1, y1, x2, y2);

        let mut qtree_visitor = DrawQtreeVisitor::new(self, &painter);
        self.engine
            .qtree
            .visit_rect(&mut qtree_visitor, &bounding_box)?;

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
    app: &'b App,
    painter: &'a egui::Painter,
    visited: u64,
}

impl<'a, 'b> DrawQtreeVisitor<'a, 'b> {
    fn new(app: &'b App, painter: &'a egui::Painter) -> Self {
        Self {
            app,
            painter,
            visited: 0,
        }
    }

    fn get_width_rect(&self, data: &quadtree::VisitData, width: f32) -> egui::Rect {
        let origin = egui::Pos2::from(self.app.pan.to_screen_uf((data.x, data.y)));
        egui::Rect::from_two_pos(origin, (origin.x + width, origin.y + width).into())
    }

    fn get_rect(&self, data: &quadtree::VisitData) -> egui::Rect {
        self.get_width_rect(data, data.width as f32 * self.app.pan.scale)
    }

    fn get_full_rect(&self, data: &quadtree::VisitData) -> egui::Rect {
        // TODO: use painter.round_to_pixel

        // adding one makes sure we cover the space between pixels
        self.get_width_rect(data, data.width as f32 * self.app.pan.scale + 1.0)
    }
}

impl<'a, 'b> quadtree::Visitor<BranchState, LeafState, anyhow::Error> for DrawQtreeVisitor<'a, 'b> {
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState,
        data: &quadtree::VisitData,
    ) -> Result<bool> {
        let should_descend = data.width as f32 * self.app.pan.scale >= 5.0;

        if !should_descend {
            let full_rect = self.get_full_rect(data);
            self.painter.rect_filled(
                full_rect,
                egui::Rounding::none(),
                egui::Color32::from_gray(100),
            );
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
            HousingTile(tiles::HousingTile { density }) => {
                self.painter.circle_filled(
                    rect.center(),
                    width / 8.0,
                    egui::Color32::from_gray(255),
                );
            }
            WorkplaceTile(tiles::WorkplaceTile { density }) => {
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

        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &BranchState,
        data: &quadtree::VisitData,
    ) -> Result<()> {
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
