use uom::si::time::{day, hour, minute};
use uom::si::u64::Time;

lazy_static::lazy_static! {
    static ref TIME_SKIPS: [(u64, &'static str); 4] = [
        (Time::new::<minute>(1).value, "+1min"),
        (Time::new::<hour>(1).value, "+1hr"),
        (Time::new::<hour>(6).value, "+6hrs"),
        (Time::new::<day>(1).value, "+1day"),
    ];
}

pub struct App {
    pub(crate) engine: engine::Engine,
    pub(crate) overlay: Overlay,
    pub(crate) options: Options,
    pub(crate) diagnostics: Diagnostics,
    pub(crate) pan: PanState,
    pub(crate) route_query: RouteQuery,
    pub(crate) congestion_analysis: CongestionAnalysis,
    pub(crate) agent_detail: AgentDetail,
}

impl App {
    fn new(mut engine: engine::Engine) -> Self {
        engine.init_trigger_queue();

        Self {
            pan: PanState::new(&engine),
            overlay: Overlay::new(),
            engine,
            options: Options::new(),
            diagnostics: Diagnostics::default(),
            route_query: RouteQuery::new(),
            congestion_analysis: CongestionAnalysis::new(),
            agent_detail: AgentDetail::new(),
        }
    }

    pub fn load_file(map: std::path::PathBuf) -> Self {
        Self::new(engine::Engine::load_file(&map).unwrap())
    }

    pub fn load_str(map: &str) -> Self {
        Self::new(engine::Engine::load(&map).unwrap())
    }

    pub fn update(&mut self, elapsed: f64) {
        // target 60 fps
        self.engine.update(elapsed, 1.0 / 60.0);
    }

    pub fn draw(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("controls")
            .resizable(false)
            .min_width(200.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.collapsing("Time", |ui| self.draw_time_state(ui));
                    ui.collapsing("Overlay", |ui| self.overlay.draw(ui));
                    ui.collapsing("Stats", |ui| self.draw_stats(ui));
                    ui.collapsing("Display options", |ui| self.options.draw(ui));

                    ui.collapsing("Diagnostics", |ui| {
                        self.diagnostics.draw(ui);
                        match self.get_hovered_pos(&ui) {
                            Some((x, y)) => ui.label(format!("Coords: {}, {}", x, y)),
                            None => ui.label("Coords: n/a"),
                        }
                    });

                    ui.collapsing("Query routes", |ui| self.draw_route_query(ui));
                    ui.collapsing("Congestion analysis", |ui| {
                        self.draw_congestion_analysis(ui)
                    });
                    ui.collapsing("Agent detail", |ui| self.draw_agent_detail(ui));
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_content(ui).unwrap();
        });
    }

    fn draw_time_state(&mut self, ui: &mut egui::Ui) {
        let time = &mut self.engine.time_state;
        ui.label(format!("Current time: {}", time.current_time));
        ui.label(
            time.current_date_time()
                .format("%a, %b %d, %Y %l:%M %P")
                .to_string(),
        );
        ui.label("Playback rate:");
        ui.add(egui::Slider::new(&mut time.playback_rate, 60..=86400));
        if ui
            .button(if time.paused { "Resume" } else { "Pause" })
            .clicked()
        {
            time.paused = !time.paused;
        }
        ui.horizontal(|ui| {
            for (skip, label) in *TIME_SKIPS {
                if ui.button(label).clicked() {
                    time.skip_by(skip);
                }
            }
        });
    }

    fn draw_stats(&mut self, ui: &mut egui::Ui) {
        if let Ok(root) = self.engine.state.qtree.get_branch(quadtree::Address::from((
            vec![],
            self.engine.state.qtree.max_depth(),
        ))) {
            ui.label(format!(
                "Population: {}",
                root.fields.population.people.total
            ));
            ui.label(format!(
                "Employment: {}",
                root.fields.employment.workers.total
            ));
        }
    }

    pub fn get_hovered_pos(&self, ui: &egui::Ui) -> Option<(u64, u64)> {
        ui.input()
            .pointer
            .hover_pos()
            .map(|pos| self.pan.to_model_fu((pos.x, pos.y)))
    }

    fn draw_route_query(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;

        if ui.button("Clear").clicked() {
            self.route_query.start_address = None;
            self.route_query.stop_address = None;
            changed = true;
        }
        ui.separator();

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
        if ui
            .checkbox(&mut self.route_query.has_car, "Has car")
            .clicked()
        {
            changed = true;
        }

        if ui.button("Pick random").clicked() {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();

            // make sure these lists are up-to-date
            self.engine.state.update_collect_tiles().unwrap();

            // for now, go from home to work
            let start = self.engine.state.collect_tiles.housing.choose(&mut rng);
            let stop = self.engine.state.collect_tiles.workplaces.choose(&mut rng);
            self.route_query.start_address = start.map(|a| *a);
            self.route_query.stop_address = stop.map(|a| *a);
            changed = true;
        }

        if ui.input().keys_down.contains(&egui::Key::A) {
            if let Some((x, y)) = self.get_hovered_pos(&ui) {
                if let Ok(start) = self.engine.state.qtree.get_address(x, y) {
                    self.route_query.start_address = Some(start);
                    changed = true;
                }
            }
        }
        if ui.input().keys_down.contains(&egui::Key::Z) {
            if let Some((x, y)) = self.get_hovered_pos(&ui) {
                if let Ok(stop) = self.engine.state.qtree.get_address(x, y) {
                    self.route_query.stop_address = Some(stop);
                    changed = true;
                }
            }
        }

        if changed {
            self.update_route_query();
        }
    }

    fn update_route_query(&mut self) {
        self.route_query.current_routes.clear();

        if let (Some(start), Some(stop)) = (
            self.route_query.start_address,
            self.route_query.stop_address,
        ) {
            let car_config = if self.route_query.has_car {
                Some(route::CarConfig::StartWithCar)
            } else {
                None
            };
            let query_input = route::QueryInput {
                start,
                end: stop,
                car_config,
            };
            match self.engine.query_route(query_input) {
                Ok(Some(route)) => self.route_query.current_routes = vec![route],
                Ok(None) => eprintln!("No route found"),
                Err(err) => eprintln!("Error querying route: {}", err),
            }
        }
    }

    fn draw_congestion_analysis(&mut self, ui: &mut egui::Ui) {
        use enum_iterator::IntoEnumIterator;
        use route::{CongestionStats, WorldState};

        let bounding_box = self.get_bounding_box(ui);
        let highway_segment_in_bounds = |highway_segment_id| {
            if self.congestion_analysis.filter_visible {
                self.engine
                    .state
                    .highways
                    .get_segments()
                    .get(&highway_segment_id)
                    .expect("missing highway segment")
                    .bounds
                    .intersects(&bounding_box)
            } else {
                true
            }
        };
        let metro_segment_in_bounds = |(metro_line, start, end)| {
            if self.congestion_analysis.filter_visible {
                self.engine
                    .state
                    .metro_lines
                    .get(&metro_line)
                    .expect("missing metro line")
                    .get_segment_bounds(start, end)
                    .expect("invalid metro segment")
                    .intersects(&bounding_box)
            } else {
                true
            }
        };

        let current_time = self.engine.time_state.current_time;
        let current_snapshot_index = self
            .engine
            .world_state_history
            .get_current_snapshot_index(current_time, true);

        // TODO: there is some duplication in here, but it's hard to pull it out because the
        // highway/metro stats types are different, and the snapshot types are different as well.

        let mut history_chart = crate::chart::Chart::new(
            self.engine
                .world_state_history
                .get_snapshots()
                .iter()
                .enumerate()
                .map(|(i, snapshot)| {
                    let history_value = match self.congestion_analysis.congestion_type {
                        CongestionType::HighwaySegments => {
                            let data = snapshot
                                .iter_highway_segments()
                                .filter(|k, v| highway_segment_in_bounds(k));
                            self.congestion_analysis.historical_quantity.get(data)
                        }
                        CongestionType::MetroSegments => {
                            let data = snapshot
                                .iter_metro_segments()
                                .filter(|k, v| metro_segment_in_bounds(k));
                            self.congestion_analysis.historical_quantity.get(data)
                        }
                    };
                    // if this snapshot is the current snapshot, display the current value as well
                    let extra = (i == current_snapshot_index).then(|| {
                        let current_value = match self.congestion_analysis.congestion_type {
                            CongestionType::HighwaySegments => {
                                let data = self
                                    .engine
                                    .world_state
                                    .iter_highway_segments()
                                    .filter(|k, v| highway_segment_in_bounds(k));
                                self.congestion_analysis.historical_quantity.get(data)
                            }
                            CongestionType::MetroSegments => {
                                let data = self
                                    .engine
                                    .world_state
                                    .iter_metro_segments()
                                    .filter(|k, v| metro_segment_in_bounds(k));
                                self.congestion_analysis.historical_quantity.get(data)
                            }
                        };
                        current_value - history_value
                    });
                    (history_value as f32, extra.map(|value| value as f32))
                })
                .collect(),
        );
        history_chart.with_labels(|i, (entry, _)| format!("{:.1}", entry));

        let histogram = match self.congestion_analysis.congestion_type {
            CongestionType::HighwaySegments => {
                let data = self
                    .engine
                    .world_state
                    .iter_highway_segments()
                    .filter(|k, v| v > 0 && highway_segment_in_bounds(k));
                data.histogram(48, 200)
            }
            CongestionType::MetroSegments => {
                let data = self
                    .engine
                    .world_state
                    .iter_metro_segments()
                    .filter(|k, v| v > 0 && metro_segment_in_bounds(k));
                data.histogram(48, 200)
            }
        };

        let mut histogram_chart =
            crate::chart::Chart::new(histogram.iter().map(|total| *total as f32).collect());
        histogram_chart.with_labels(|i, entry| format!("{}", entry as f64));

        egui::ComboBox::from_id_source("congestion_analysis_type")
            .selected_text(self.congestion_analysis.congestion_type.label())
            .show_ui(ui, |ui| {
                for congestion_type in CongestionType::into_enum_iter() {
                    ui.selectable_value(
                        &mut self.congestion_analysis.congestion_type,
                        congestion_type,
                        congestion_type.label(),
                    );
                }
            });

        ui.checkbox(
            &mut self.congestion_analysis.filter_visible,
            "Filter visible",
        );

        ui.label("Historical congestion");
        ui.label(format!("Scale {:.1}", history_chart.rounded_max_entry,));
        ui.add(history_chart);

        egui::ComboBox::from_id_source("congestion_analysis_historical_quantity")
            .selected_text(self.congestion_analysis.historical_quantity.label())
            .show_ui(ui, |ui| {
                for quantity in CongestionHistoricalQuantity::into_enum_iter() {
                    ui.selectable_value(
                        &mut self.congestion_analysis.historical_quantity,
                        quantity,
                        quantity.label(),
                    );
                }
            });

        ui.label("Current histogram");
        ui.label(format!("Scale: {:.1}", histogram_chart.rounded_max_entry));
        ui.add(histogram_chart);
    }

    fn draw_agent_detail(&mut self, ui: &mut egui::Ui) {
        match self.agent_detail {
            AgentDetail::Empty => {
                if ui.button("Pick tile").clicked() {
                    self.agent_detail = AgentDetail::Querying;
                }
            }
            AgentDetail::Querying => {
                if ui.button("Clear").clicked() {
                    self.agent_detail = AgentDetail::Empty;
                }
                ui.separator();

                ui.label("<waiting for tile selection>");
            }
            AgentDetail::Query { address } => {
                use tiles::TileType;

                if ui.button("Clear").clicked() {
                    self.agent_detail = AgentDetail::Empty;
                }
                ui.separator();

                let leaf = self.engine.state.qtree.get_leaf(address).unwrap();
                // TODO: replace with if-let chain once stabilized
                let agents = leaf.tile.query_agents().and_then(|agents| {
                    if agents.len() > 0 {
                        Some(agents)
                    } else {
                        None
                    }
                });
                if let Some(agents) = agents {
                    ui.label(format!("Selected tile has {} agent(s):", agents.len()));

                    for id in agents {
                        let agent = self.engine.agents.get(id).expect("missing agent");
                        if ui.button(format!("Agent #{}", id)).clicked() {
                            self.agent_detail = AgentDetail::Selected { id: *id };
                        }
                    }
                } else {
                    ui.label("Selected tile has no agents");
                }
            }
            AgentDetail::Selected { id } => {
                if ui.button("Clear").clicked() {
                    self.agent_detail = AgentDetail::Empty;
                }
                ui.separator();

                self.draw_agent_info(ui, id);
            }
        }
    }

    fn draw_agent_info(&mut self, ui: &mut egui::Ui, id: u64) {
        let agent = self.engine.agents.get(&id).expect("missing agent");

        ui.label(format!("Agent #{}:", id));
        ui.label(format!(
            "Age: {}",
            agent.data.age(self.engine.time_state.current_date())
        ));
        ui.label(format!(
            "Education: {} ({} years)",
            agent.data.education_degree(),
            agent.data.years_of_education
        ));

        let (home_x, home_y) = agent.housing.to_xy();
        ui.label(format!("Home: ({}, {})", home_x, home_y));

        match agent.workplace {
            Some(workplace) => {
                let (work_x, work_y) = workplace.to_xy();
                ui.label(format!("Work: ({}, {})", work_x, work_y));
            }
            None => {
                ui.label("Work: n/a");
            }
        }

        // TODO: use some better way to format durations. chrono::Duration itself is not
        // formattable.
        if let Some(average_commute) = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
            agent.average_commute_length() as u32,
            0,
        ) {
            ui.label(format!(
                "Average commute: {}",
                average_commute.format("%H:%M:%S"),
            ));
        }

        if let Some(workplace) = agent.workplace {
            if ui.button("Show commute").clicked() {
                self.route_query.start_address = Some(agent.housing);
                self.route_query.stop_address = Some(workplace);
                self.update_route_query();
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Overlay {
    pub field: Option<crate::field_overlay::FieldType>,
    pub traffic: bool,
}

impl Overlay {
    fn new() -> Self {
        Self {
            field: None,
            traffic: false,
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        use enum_iterator::IntoEnumIterator;

        ui.radio_value(&mut self.field, None, "None");
        for field_type in crate::field_overlay::FieldType::into_enum_iter() {
            ui.radio_value(&mut self.field, Some(field_type), field_type.label());
        }
        ui.checkbox(&mut self.traffic, "Traffic");
    }
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
            min_tile_size: 2,
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
    pub agents: u64,
}

impl Diagnostics {
    fn draw(&self, ui: &mut egui::Ui) {
        ui.label(format!("Frame rate: {:.1}", self.frame_rate));
        ui.label(format!("Tiles: {}", self.tiles));
        ui.label(format!("Metro vertices: {}", self.metro_vertices));
        ui.label(format!("Highway vertices: {}", self.highway_vertices));
        ui.label(format!("Agents: {}", self.agents));
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
    fn new(engine: &engine::Engine) -> Self {
        // TODO: pass through actual screen dimensions?
        let (width, height) = (1920.0, 1080.0);

        let min_dim = f32::min(width, height);
        let model_width = engine.state.qtree.width() as f32;

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
    pub current_routes: Vec<route::Route>,
}

impl RouteQuery {
    fn new() -> Self {
        Self {
            start_address: None,
            stop_address: None,
            start_selection: false,
            stop_selection: false,
            has_car: true,
            current_routes: Vec::new(),
        }
    }
}

#[derive(Debug, enum_iterator::IntoEnumIterator, PartialEq, Copy, Clone)]
pub(crate) enum CongestionType {
    HighwaySegments,
    MetroSegments,
}

impl CongestionType {
    fn label(&self) -> &'static str {
        match self {
            Self::HighwaySegments => "Highways",
            Self::MetroSegments => "Metros",
        }
    }
}

#[derive(Debug, enum_iterator::IntoEnumIterator, PartialEq, Copy, Clone)]
pub(crate) enum CongestionHistoricalQuantity {
    Sum,
    Mean,
    Rms,
}

impl CongestionHistoricalQuantity {
    fn label(&self) -> &'static str {
        match self {
            Self::Sum => "Sum",
            Self::Mean => "Mean",
            Self::Rms => "RMS",
        }
    }

    fn get<'a, K>(&self, congestion_stats: route::CongestionIterator<'a, K>) -> f32 {
        use route::CongestionStats;
        match self {
            Self::Sum => congestion_stats.sum() as f32,
            Self::Mean => congestion_stats.mean() as f32,
            Self::Rms => congestion_stats.rms() as f32,
        }
    }
}

pub(crate) struct CongestionAnalysis {
    pub filter_visible: bool,
    pub congestion_type: CongestionType,
    pub historical_quantity: CongestionHistoricalQuantity,
}

impl CongestionAnalysis {
    fn new() -> Self {
        Self {
            filter_visible: false,
            congestion_type: CongestionType::HighwaySegments,
            historical_quantity: CongestionHistoricalQuantity::Rms,
        }
    }
}

pub(crate) enum AgentDetail {
    /// no selection
    Empty,
    /// waiting for user to select a tile
    Querying,
    /// user selected a tile, which has zero or more agents
    Query { address: quadtree::Address },
    /// user picked one of the agents
    Selected { id: u64 },
}

impl AgentDetail {
    fn new() -> Self {
        Self::Empty
    }
}
