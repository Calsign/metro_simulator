use std::rc::Rc;
use std::sync::{Arc, Mutex};

static DEFAULT_WINDOW_SIZE: (f64, f64) = (1920.0, 1080.0);
static WINDOW_TITLE: &str = "Metro Simulator";
static DEFAULT_CONFIG: &str = "config/debug.toml";

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(short, long)]
    load: Option<std::path::PathBuf>,
}

fn main() {
    use clap::Parser;
    let args = Args::parse();

    let window = druid::WindowDesc::new(build_root_widget())
        .title(WINDOW_TITLE)
        .window_size(DEFAULT_WINDOW_SIZE);

    let engine = Arc::new(Mutex::new(match args.load {
        Some(path) => engine::state::State::load_file(&path).unwrap(),
        None => engine::state::State::new(
            engine::config::Config::load_file(&std::path::PathBuf::from(DEFAULT_CONFIG)).unwrap(),
        ),
    }));

    let state = State {
        content: ContentState::new(engine.clone()),
        metro_lines: MetroLinesState::new(engine),
    };

    druid::AppLauncher::with_window(window)
        .launch(state)
        .unwrap();
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct State {
    content: ContentState,
    metro_lines: MetroLinesState,
}

fn build_root_widget() -> impl druid::Widget<State> {
    use druid::WidgetExt;
    druid::widget::Flex::row()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(
            druid::widget::Flex::column()
                .with_flex_child(
                    druid::widget::Maybe::new(build_detail_panel, build_empty_panel)
                        .lens(ContentState::current_leaf)
                        .lens(State::content)
                        .expand()
                        .padding((20.0, 20.0)),
                    1.0,
                )
                .with_default_spacer()
                .with_flex_child(
                    build_metro_lines_panel().expand().padding((20.0, 20.0)),
                    1.0,
                )
                .fix_width(300.0),
        )
        .with_flex_child(Content {}.lens(State::content), 1.0)
        .with_child(
            druid::widget::Flex::column().with_child(
                build_menu_panel()
                    .expand()
                    .padding((20.0, 20.0))
                    .fix_width(300.0),
            ),
        )
}

fn tile_types() -> Vec<(&'static str, std::mem::Discriminant<tiles::Tile>)> {
    use std::mem::discriminant;
    vec![
        ("Empty", discriminant(&tiles::EmptyTile {}.into())),
        (
            "Housing",
            discriminant(&tiles::HousingTile { density: 1 }.into()),
        ),
        (
            "Workplace",
            discriminant(&tiles::WorkplaceTile { density: 1 }.into()),
        ),
    ]
}

fn build_detail_panel() -> impl druid::Widget<CurrentLeafState> {
    use druid::WidgetExt;
    druid::widget::Flex::column()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(druid::widget::Label::dynamic(
            |state: &CurrentLeafState, env: &druid::Env| {
                use tiles::TileType;
                format!("Tile type: {}", state.leaf.tile.name())
            },
        ))
        .with_default_spacer()
        .with_child(druid::widget::Label::dynamic(
            |state: &CurrentLeafState, env: &druid::Env| {
                format!("Address: {:?}", (*state.address).clone().to_vec())
            },
        ))
        .with_default_spacer()
        .with_child(
            druid::widget::TextBox::multiline()
                .fix_width(200.0)
                .lens(CurrentLeafState::edited_data),
        )
        .with_default_spacer()
        .with_child(druid::widget::RadioGroup::new(tile_types()).lens(CurrentLeafState::tile_type))
        .with_default_spacer()
        .with_child(druid::widget::Button::new("Update").on_click(
            |ctx: &mut druid::EventCtx, state: &mut CurrentLeafState, env: &druid::Env| {
                // NOTE: we need to do some juggling to adhere to borrowing rules
                let mut update = false;
                {
                    let mut engine = state.engine.lock().unwrap();
                    match engine.set_leaf_data(
                        (*state.address).clone(),
                        &state.edited_data,
                        engine::state::SerdeFormat::Toml,
                    ) {
                        Ok(()) => {
                            // update with new state
                            update = true;
                        }
                        Err(err) => println!("Error updating leaf: {:?}", err),
                    }
                }
                if update {
                    // TODO: this is gross
                    let engine1 = state.engine.clone();
                    let engine2 = state.engine.clone();
                    let engine = engine1.lock().unwrap();
                    *state = CurrentLeafState::new((*state.address).clone(), &engine, engine2);
                }
            },
        ))
}

fn build_metro_lines_panel() -> impl druid::Widget<State> {
    use druid::WidgetExt;
    druid::widget::Flex::column()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(
            druid::widget::Button::new("New Metro Line")
                .on_click(
                    |ctx: &mut druid::EventCtx, state: &mut MetroLinesState, env: &druid::Env| {
                        let mut engine = state.engine.lock().unwrap();
                        engine.add_metro_line(String::from("Metro Line"));
                    },
                )
                .lens(State::metro_lines),
        )
        .with_default_spacer()
        .with_child(
            druid::widget::Scroll::new(druid::widget::List::new(|| {
                druid::widget::Flex::row()
                    .with_child(
                        druid::widget::Painter::new(|ctx, data: &MetroLineData, env| {
                            use druid::RenderContext;
                            let metro_line = data.metro_line.lock().unwrap();
                            let center = (ctx.size().width / 2.0, ctx.size().height / 2.0);
                            let circle = druid::kurbo::Circle::new(
                                center,
                                f64::min(ctx.size().width, ctx.size().height) / 4.0,
                            );
                            let color = metro_line.color;
                            ctx.fill(
                                circle,
                                &druid::Color::rgb8(color.red, color.green, color.blue),
                            );
                        })
                        .fix_size(20.0, 20.0),
                    )
                    .with_child(druid::widget::TextBox::new().lens(druid::lens::Map::new(
                        |data: &MetroLineData| data.metro_line.lock().unwrap().name.clone(),
                        |data: &mut MetroLineData, inner: String| {
                            let mut metro_line = data.metro_line.lock().unwrap();
                            metro_line.name = inner;
                        },
                    )))
            }))
            .vertical()
            .lens(State::metro_lines),
        )
}

fn build_menu_panel() -> impl druid::Widget<State> {
    druid::widget::Flex::column()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(druid::widget::Button::new("Save").on_click(
            |ctx: &mut druid::EventCtx, state: &mut State, env: &druid::Env| {
                let engine = state.content.engine.lock().unwrap();
                let timestamp = chrono::offset::Local::now();
                let path = format!(
                    "/tmp/metro_simulator_{}.json",
                    timestamp.format("%Y-%m-%d_%H-%M-%S"),
                );
                engine.dump_file(&std::path::PathBuf::from(&path)).unwrap();
                println!("Saved to {}", path);
            },
        ))
}

fn build_empty_panel() -> impl druid::Widget<()> {
    druid::widget::Flex::column()
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct MetroLinesState {
    engine: Arc<Mutex<engine::state::State>>,
}

impl MetroLinesState {
    fn new(engine: Arc<Mutex<engine::state::State>>) -> Self {
        Self {
            engine: engine.clone(),
        }
    }
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct MetroLineData {
    metro_line: Arc<Mutex<metro::MetroLine>>,
}

impl MetroLineData {
    fn new(metro_line: &metro::MetroLine) -> Self {
        Self {
            metro_line: Arc::new(Mutex::new(metro_line.clone())),
        }
    }
}

impl druid::widget::ListIter<MetroLineData> for MetroLinesState {
    fn for_each(&self, mut cb: impl FnMut(&MetroLineData, usize)) {
        let engine = self.engine.lock().unwrap();
        for (i, (_, metro_line)) in engine.metro_lines.iter().enumerate() {
            // TODO: this clone is disgusting
            let data = MetroLineData::new(metro_line);
            cb(&data, i);
        }
    }

    fn for_each_mut(&mut self, mut cb: impl FnMut(&mut MetroLineData, usize)) {
        let mut engine = self.engine.lock().unwrap();
        for (i, (_, metro_line)) in engine.metro_lines.iter_mut().enumerate() {
            // TODO: this double clone is even more disgusting
            let mut data = MetroLineData::new(metro_line);
            cb(&mut data, i);
            *metro_line = data.metro_line.lock().unwrap().clone();
        }
    }

    fn data_len(&self) -> usize {
        self.engine.lock().unwrap().metro_lines.len()
    }
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct CurrentLeafState {
    address: Rc<quadtree::Address>,
    leaf: Rc<engine::state::LeafState>,
    tile_type: std::mem::Discriminant<tiles::Tile>,
    data: String,
    edited_data: String,

    engine: Arc<Mutex<engine::state::State>>,
}

impl CurrentLeafState {
    fn new(
        address: quadtree::Address,
        engine: &engine::state::State,
        engine_clone: Arc<Mutex<engine::state::State>>,
    ) -> Self {
        let leaf = engine.qtree.get_leaf(address.clone()).unwrap();
        let data = engine
            .get_leaf_data(address.clone(), engine::state::SerdeFormat::Toml)
            .unwrap();
        Self {
            address: Rc::new(address),
            leaf: Rc::new(leaf.clone()),
            tile_type: std::mem::discriminant(&leaf.tile),
            data: data.clone(),
            edited_data: data.clone(),
            engine: engine_clone,
        }
    }
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct ContentState {
    engine: Arc<Mutex<engine::state::State>>,

    scale: f64,
    tx: f64,
    ty: f64,

    min_scale: f64,
    max_scale: f64,

    mouse_pos: Option<druid::Point>,

    current_leaf: Option<CurrentLeafState>,
}

impl ContentState {
    pub fn new(engine: Arc<Mutex<engine::state::State>>) -> Self {
        let engine_locked = engine.lock().unwrap();

        let (mut width, height) = DEFAULT_WINDOW_SIZE as (f64, f64);
        // account for the two side panels
        // TODO: make this less gross
        width -= 600.0;

        let min_dim = f64::min(width, height);
        let model_width = engine_locked.qtree.width() as f64;

        let scale = min_dim / model_width / 2.0;
        let tx = width / 2.0 - model_width * scale / 2.0;
        let ty = height / 2.0 - model_width * scale / 2.0;

        let min_scale = min_dim / model_width / 2.0;
        let max_scale = 100.0;

        Self {
            engine: engine.clone(),
            scale,
            tx,
            ty,
            min_scale,
            max_scale,
            mouse_pos: None,
            current_leaf: None,
        }
    }

    pub fn to_screen(&self, (x, y): (u64, u64)) -> (f64, f64) {
        (
            x as f64 * self.scale + self.tx,
            y as f64 * self.scale + self.ty,
        )
    }

    pub fn to_model(&self, (x, y): (f64, f64)) -> (u64, u64) {
        (
            ((x - self.tx) / self.scale) as u64,
            ((y - self.ty) / self.scale) as u64,
        )
    }
}

struct Content {}

impl druid::Widget<ContentState> for Content {
    fn event(
        &mut self,
        ctx: &mut druid::EventCtx,
        event: &druid::Event,
        state: &mut ContentState,
        env: &druid::Env,
    ) {
        use druid::Event::*;
        match event {
            MouseDown(mouse) | MouseUp(mouse) if mouse.buttons.has_left() => {
                state.mouse_pos = None;
            }
            MouseMove(mouse) if mouse.buttons.has_left() => {
                if let Some(old) = state.mouse_pos {
                    let diff = mouse.pos - old;
                    state.tx += diff.x;
                    state.ty += diff.y;
                    ctx.request_paint();
                }
                state.mouse_pos = Some(mouse.pos)
            }
            Wheel(mouse) => {
                let new_scale = f64::max(
                    f64::min(
                        state.scale * 1.1_f64.powf(-mouse.wheel_delta.y / 10.0),
                        state.max_scale,
                    ),
                    state.min_scale,
                );

                let mx = mouse.pos.x;
                let my = mouse.pos.y;

                // Zoom centered on mouse
                state.tx = (mx * state.scale - mx * new_scale + state.tx * new_scale) / state.scale;
                state.ty = (my * state.scale - my * new_scale + state.ty * new_scale) / state.scale;

                state.scale = new_scale;
                ctx.request_paint();
            }
            MouseDown(mouse) if mouse.buttons.has_right() => {
                let mut engine = state.engine.lock().unwrap();
                let (mx, my) = state.to_model(mouse.pos.into());
                let w = engine.qtree.width();
                if mx > 0 && mx < w && my > 0 && my < w {
                    let address = engine.qtree.get_address(mx, my).unwrap();
                    if address.depth() < engine.qtree.max_depth() {
                        use engine::state::{BranchState, LeafState};
                        engine
                            .qtree
                            .split(
                                address.clone(),
                                BranchState {},
                                quadtree::QuadMap::new(
                                    LeafState::default(),
                                    LeafState::default(),
                                    LeafState::default(),
                                    LeafState::default(),
                                ),
                            )
                            .unwrap();
                        if let Some(current_leaf) = &state.current_leaf {
                            if *current_leaf.address == address {
                                state.current_leaf = None;
                            }
                        }
                        ctx.request_paint();
                    }
                }
            }
            MouseDown(mouse) if mouse.buttons.has_middle() => {
                let engine = state.engine.lock().unwrap();
                let (mx, my) = state.to_model(mouse.pos.into());
                let w = engine.qtree.width();
                if mx > 0 && mx < w && my > 0 && my < w {
                    let address = engine.qtree.get_address(mx, my).unwrap();
                    state.current_leaf = Some(CurrentLeafState::new(
                        address,
                        &engine,
                        state.engine.clone(),
                    ));
                } else {
                    state.current_leaf = None;
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut druid::LifeCycleCtx<'_, '_>,
        event: &druid::LifeCycle,
        state: &ContentState,
        env: &druid::Env,
    ) {
    }

    fn update(
        &mut self,
        ctx: &mut druid::UpdateCtx<'_, '_>,
        old_data: &ContentState,
        state: &ContentState,
        env: &druid::Env,
    ) {
    }

    fn layout(
        &mut self,
        ctx: &mut druid::LayoutCtx<'_, '_>,
        bc: &druid::BoxConstraints,
        state: &ContentState,
        env: &druid::Env,
    ) -> druid::Size {
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(DEFAULT_WINDOW_SIZE)
        }
    }

    fn paint(&mut self, ctx: &mut druid::PaintCtx, state: &ContentState, env: &druid::Env) {
        let engine = state.engine.lock().unwrap();

        let (x1, y1) = state.to_model((0.0, 0.0));
        let (x2, y2) = state.to_model(ctx.size().into());

        let mut qtree_visitor = PaintQtreeVisitor { ctx, env, state };
        engine
            .qtree
            .visit_rect(&mut qtree_visitor, &quadtree::Rect::corners(x1, y1, x2, y2))
            .unwrap();

        for (id, metro_line) in engine.metro_lines.iter() {
            let mut spline_visitor = PaintSplineVisitor {
                ctx,
                env,
                state,
                last_point: None,
            };
            metro_line
                .visit_spline(&mut spline_visitor, state.scale)
                .unwrap();
        }
    }
}

struct PaintQtreeVisitor<'a, 'b, 'c, 'd, 'e, 'f> {
    ctx: &'a mut druid::PaintCtx<'c, 'd, 'e>,
    env: &'b druid::Env,
    state: &'f ContentState,
}

impl<'a, 'b, 'c, 'd, 'e, 'f>
    quadtree::Visitor<engine::state::BranchState, engine::state::LeafState, anyhow::Error>
    for PaintQtreeVisitor<'a, 'b, 'c, 'd, 'e, 'f>
{
    fn visit_branch(
        &mut self,
        branch: &engine::state::BranchState,
        data: &quadtree::VisitData,
    ) -> anyhow::Result<bool> {
        Ok(data.width as f64 * self.state.scale >= 2.0)
    }

    fn visit_leaf(
        &mut self,
        leaf: &engine::state::LeafState,
        data: &quadtree::VisitData,
    ) -> anyhow::Result<()> {
        use druid::RenderContext;

        let width = data.width as f64 * self.state.scale;
        let rect =
            druid::Rect::from_origin_size(self.state.to_screen((data.x, data.y)), (width, width));
        let mut stroke_weight = 1.0;
        if let Some(current_leaf) = &self.state.current_leaf {
            if *current_leaf.address == data.address {
                stroke_weight = 5.0;
            }
        }
        self.ctx
            .stroke(rect, &druid::Color::grey8(200), stroke_weight);

        use tiles::Tile::*;
        match &leaf.tile {
            WaterTile(tiles::WaterTile {}) => {
                self.ctx.fill(rect, &druid::Color::rgb8(0, 0, 150));
            }
            HousingTile(tiles::HousingTile { density }) => {
                let circle = druid::kurbo::Circle::new(rect.center(), width / 8.0);
                self.ctx.fill(circle, &druid::Color::grey8(255));
            }
            WorkplaceTile(tiles::WorkplaceTile { density }) => {
                let triangle = triangle(
                    rect.center().into(),
                    width / 6.0,
                    -std::f64::consts::FRAC_PI_2,
                );
                self.ctx.fill(&triangle[..], &druid::Color::grey8(255));
            }
            MetroStationTile(tiles::MetroStationTile { x, y, ids }) => {
                let point = self.state.to_screen((data.x + x, data.y + y));
                let circle = druid::kurbo::Circle::new(point, width / 20.0);
                self.ctx.stroke(circle, &druid::Color::grey8(255), 1.0);
            }
            _ => (),
        }

        Ok(())
    }
}

fn triangle((x, y): (f64, f64), radius: f64, theta: f64) -> [druid::kurbo::PathEl; 4] {
    use druid::kurbo::PathEl::*;
    use std::f64::consts::PI;
    let sides = 3;
    let mut points = [(0.0, 0.0); 3];
    for i in 0..sides {
        let t = (PI * 2.0) / sides as f64 * i as f64 + theta;
        points[i] = (x + t.cos() * radius, y + t.sin() * radius)
    }
    [
        MoveTo(points[0].into()),
        LineTo(points[1].into()),
        LineTo(points[2].into()),
        ClosePath,
    ]
}

struct PaintSplineVisitor<'a, 'b, 'c, 'd, 'e, 'f> {
    ctx: &'a mut druid::PaintCtx<'c, 'd, 'e>,
    env: &'b druid::Env,
    state: &'f ContentState,

    last_point: Option<(f64, f64)>,
}

impl<'a, 'b, 'c, 'd, 'e, 'f> metro::SplineVisitor<anyhow::Error>
    for PaintSplineVisitor<'a, 'b, 'c, 'd, 'e, 'f>
{
    fn visit(
        &mut self,
        line: &metro::MetroLine,
        vertex: cgmath::Vector2<f64>,
        t: f64,
    ) -> Result<(), anyhow::Error> {
        use druid::RenderContext;

        let point = (
            vertex.x * self.state.scale + self.state.tx,
            vertex.y * self.state.scale + self.state.ty,
        );

        if let Some(last_point) = self.last_point {
            self.ctx.stroke(
                druid::kurbo::Line::new(last_point, point),
                &druid::Color::rgb8(line.color.red, line.color.green, line.color.blue),
                2.0,
            );
        };

        self.last_point = Some(point);
        Ok(())
    }
}
