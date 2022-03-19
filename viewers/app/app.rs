pub struct App {
    state: State,
}

impl App {
    pub fn load_file(map: std::path::PathBuf) -> Self {
        Self {
            state: State::new(engine::state::State::load_file(&map).unwrap()),
        }
    }

    pub fn load_str(map: &str) -> Self {
        Self {
            state: State::new(engine::state::State::load(&map).unwrap()),
        }
    }

    pub fn update(&mut self) {
        // todo
    }

    pub fn draw(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello, world!");
            if ui.button("add one").clicked() {}
        });
    }
}

struct State {
    engine: engine::state::State,
    pan: PanState,
}

impl State {
    fn new(engine: engine::state::State) -> Self {
        Self {
            pan: PanState::new(&engine),
            engine,
        }
    }
}

struct PanState {
    scale: f64,
    tx: f64,
    ty: f64,

    min_scale: f64,
    max_scale: f64,
}

impl PanState {
    fn new(engine: &engine::state::State) -> Self {
        // TODO: pass through actual screen dimensions?
        let (width, height) = (1920.0, 1080.0);

        let min_dim = f64::min(width, height);
        let model_width = engine.qtree.width() as f64;

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

    fn to_screen_uf(&self, (x, y): (u64, u64)) -> (f64, f64) {
        self.to_screen_ff((x as f64, y as f64))
    }

    fn to_screen_ff(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (x * self.scale + self.tx, y * self.scale + self.ty)
    }

    fn to_model_fu(&self, (x, y): (f64, f64)) -> (u64, u64) {
        let (mx, my) = self.to_model_fu((x, y));
        (mx as u64, my as u64)
    }

    fn to_model_ff(&self, (x, y): (f64, f64)) -> (f64, f64) {
        ((x - self.tx) / self.scale, (y - self.ty) / self.scale)
    }
}
