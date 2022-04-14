pub struct App {
    pub(crate) engine: engine::state::State,
    pub(crate) field: Option<FieldType>,
    pub(crate) options: Options,
    pub(crate) diagnostics: Diagnostics,
    pub(crate) pan: PanState,
    pub(crate) routes: Vec<route::Route>,
    pub(crate) route_query: RouteQuery,
}

impl App {
    fn new(mut engine: engine::state::State) -> Self {
        // TODO: re-run this when the qtree updates
        engine.update_fields().unwrap();

        Self {
            pan: PanState::new(&engine),
            field: None,
            engine,
            options: Options::new(),
            diagnostics: Diagnostics::default(),
            routes: Vec::new(),
            route_query: RouteQuery::new(),
        }
    }

    pub fn load_file(map: std::path::PathBuf) -> Self {
        Self::new(engine::state::State::load_file(&map).unwrap())
    }

    pub fn load_str(map: &str) -> Self {
        Self::new(engine::state::State::load(&map).unwrap())
    }

    pub fn update(&mut self) {
        // todo
    }

    pub fn draw(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("controls")
            .resizable(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label("Field:");
                    ui.radio_value(&mut self.field, None, "None");
                    ui.radio_value(&mut self.field, Some(FieldType::Population), "Population");
                    ui.radio_value(&mut self.field, Some(FieldType::Employment), "Employment");
                    ui.radio_value(&mut self.field, Some(FieldType::LandValue), "Land value");

                    ui.add_space(10.0);
                    self.options.draw(ui);
                    ui.add_space(10.0);
                    self.diagnostics.draw(ui);

                    if let Some((x, y)) = self.get_hovered_pos(&ui) {
                        ui.add_space(10.0);
                        ui.label(format!("Coords: {}, {}", x, y));
                    }

                    ui.add_space(10.0);
                    ui.collapsing("Query routes", |ui| self.draw_route_query(ui));
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_content(ui).unwrap();
        });
    }

    pub fn get_hovered_pos(&self, ui: &egui::Ui) -> Option<(u64, u64)> {
        ui.input()
            .pointer
            .hover_pos()
            .map(|pos| self.pan.to_model_fu((pos.x, pos.y)))
    }

    fn draw_route_query(&mut self, ui: &mut egui::Ui) {
        match self.route_query.start_address {
            Some(start) => {
                let (x, y) = start.to_xy();
                ui.label(format!("[a] Start: {}, {}", x, y));
            }
            None => {
                ui.label("[a] No start selected");
            }
        }
        match self.route_query.stop_address {
            Some(stop) => {
                let (x, y) = stop.to_xy();
                ui.label(format!("[z] Stop: {}, {}", x, y));
            }
            None => {
                ui.label("[z] No stop selected");
            }
        }
        ui.checkbox(&mut self.route_query.has_car, "Has car");

        if ui.input().keys_down.contains(&egui::Key::A) {
            if let Some((x, y)) = self.get_hovered_pos(&ui) {
                if let Ok(start) = self.engine.qtree.get_address(x, y) {
                    self.route_query.start_address = Some(start);
                }
            }
        }
        if ui.input().keys_down.contains(&egui::Key::Z) {
            if let Some((x, y)) = self.get_hovered_pos(&ui) {
                if let Ok(stop) = self.engine.qtree.get_address(x, y) {
                    self.route_query.stop_address = Some(stop);
                }
            }
        }

        if ui.button("Plot route").clicked() {
            if let (Some(start), Some(stop)) = (
                self.route_query.start_address,
                self.route_query.stop_address,
            ) {
                // TODO: move base graph computation into engine
                let mut base_graph = self.engine.construct_base_route_graph().unwrap();
                let query_input = route::QueryInput {
                    base_graph: &mut base_graph,
                    start,
                    end: stop,
                    state: &route::WorldState::new(),
                    car_config: if self.route_query.has_car {
                        Some(route::CarConfig::StartWithCar)
                    } else {
                        None
                    },
                };
                if let Ok(Some(route)) = route::best_route(query_input) {
                    self.routes = vec![route];
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum FieldType {
    Population,
    Employment,
    LandValue,
}

#[derive(Debug)]
pub(crate) struct Options {
    pub min_tile_size: u32,
    pub spline_resolution: u32,
    pub field_resolution: u32,
}

impl Options {
    fn new() -> Self {
        Self {
            min_tile_size: 5,
            spline_resolution: 5,
            field_resolution: 10,
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        ui.label("Min tile size:");
        ui.add(egui::Slider::new(&mut self.min_tile_size, 1..=100));
        ui.label("Spline resolution:");
        ui.add(egui::Slider::new(&mut self.spline_resolution, 1..=100));
        ui.label("Field resolution:");
        ui.add(egui::Slider::new(&mut self.field_resolution, 3..=100));
    }
}

#[derive(Debug, Default)]
pub(crate) struct Diagnostics {
    pub frame_rate: f64,
    pub tiles: u64,
    pub metro_vertices: u64,
    pub highway_vertices: u64,
}

impl Diagnostics {
    fn draw(&self, ui: &mut egui::Ui) {
        ui.label(format!("Frame rate: {:.1}", self.frame_rate));
        ui.label(format!("Tiles: {}", self.tiles));
        ui.label(format!("Metro vertices: {}", self.metro_vertices));
        ui.label(format!("Highway vertices: {}", self.highway_vertices));
    }
}

pub(crate) struct PanState {
    pub scale: f32,
    pub tx: f32,
    pub ty: f32,

    pub min_scale: f32,
    pub max_scale: f32,
}

impl PanState {
    fn new(engine: &engine::state::State) -> Self {
        // TODO: pass through actual screen dimensions?
        let (width, height) = (1920.0, 1080.0);

        let min_dim = f32::min(width, height);
        let model_width = engine.qtree.width() as f32;

        // TODO: this logic is duplicated in //viewers/editor
        let scale = min_dim / model_width / 2.0;
        let tx = width / 2.0 - model_width * scale / 2.0;
        let ty = height / 2.0 - model_width * scale / 2.0;

        let min_scale = min_dim / model_width / 2.0;
        let max_scale = 100.0;

        Self {
            scale,
            tx,
            ty,
            min_scale,
            max_scale,
        }
    }

    pub fn to_screen_uf(&self, (x, y): (u64, u64)) -> (f32, f32) {
        self.to_screen_ff((x as f32, y as f32))
    }

    pub fn to_screen_ff(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (x * self.scale + self.tx, y * self.scale + self.ty)
    }

    pub fn to_model_fu(&self, (x, y): (f32, f32)) -> (u64, u64) {
        let (mx, my) = self.to_model_ff((x, y));
        (mx as u64, my as u64)
    }

    pub fn to_model_ff(&self, (x, y): (f32, f32)) -> (f32, f32) {
        ((x - self.tx) / self.scale, (y - self.ty) / self.scale)
    }
}

pub(crate) struct RouteQuery {
    pub start_address: Option<quadtree::Address>,
    pub stop_address: Option<quadtree::Address>,
    pub start_selection: bool,
    pub stop_selection: bool,
    pub has_car: bool,
}

impl RouteQuery {
    fn new() -> Self {
        Self {
            start_address: None,
            stop_address: None,
            start_selection: false,
            stop_selection: false,
            has_car: false,
        }
    }
}
