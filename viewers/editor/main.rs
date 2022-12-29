use std::rc::Rc;
use std::sync::{Arc, Mutex};

static DEFAULT_WINDOW_SIZE: (f64, f64) = (1920.0, 1080.0);
static WINDOW_TITLE: &str = "Metro Simulator";
static DEFAULT_CONFIG: &str = "configs/debug.toml";

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
        Some(path) => engine::Engine::load_file(&path).unwrap(),
        None => engine::Engine::new(
            state::Config::load_file(&std::path::PathBuf::from(DEFAULT_CONFIG)).unwrap(),
        ),
    }));

    // TODO: re-run this when the qtree updates
    engine.lock().unwrap().update_fields().unwrap();

    let state = State {
        engine: engine.clone(),
        metro_lines: MetroLinesState::new(engine.clone()),
        content: ContentState::new(engine),
        current_leaf: None,
        current_field: FieldType::None,
        show_qtree: true,
        show_metros: true,
        show_metro_keys: false,
        show_metro_directions: false,
        show_highways: true,
        show_highway_keys: false,
        show_highway_directions: false,
    };

    druid::AppLauncher::with_window(window)
        .launch(state)
        .unwrap();
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct State {
    engine: Arc<Mutex<engine::Engine>>,
    metro_lines: MetroLinesState,
    content: ContentState,
    current_leaf: Option<CurrentLeafState>,
    current_field: FieldType,
    show_qtree: bool,
    show_metros: bool,
    show_metro_keys: bool,
    show_metro_directions: bool,
    show_highways: bool,
    show_highway_keys: bool,
    show_highway_directions: bool,
}

fn build_root_widget() -> impl druid::Widget<State> {
    use druid::WidgetExt;
    druid::widget::Flex::row()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(
            druid::widget::Flex::column()
                .with_flex_child(
                    druid::widget::Maybe::new(build_detail_panel, build_empty_panel)
                        .lens(State::current_leaf)
                        .expand()
                        .padding((20.0, 20.0)),
                    1.0,
                )
                .with_default_spacer()
                .with_flex_child(
                    build_metro_lines_panel().expand().padding((20.0, 20.0)),
                    1.0,
                )
                .fix_width(300.0)
                .background(druid::Color::grey(0.2))
                .expand_height(),
        )
        .with_flex_child(Content {}, 1.0)
        .with_child(
            druid::widget::Flex::column()
                .with_child(build_menu_panel().expand().padding((20.0, 20.0)))
                .fix_width(300.0)
                .background(druid::Color::grey(0.2))
                .expand_height(),
        )
}

fn tile_types() -> Vec<(&'static str, std::mem::Discriminant<tiles::Tile>)> {
    use std::mem::discriminant;
    vec![
        ("Empty", discriminant(&tiles::EmptyTile {}.into())),
        (
            "Housing",
            discriminant(
                &tiles::HousingTile {
                    density: 1,
                    agents: vec![],
                }
                .into(),
            ),
        ),
        (
            "Workplace",
            discriminant(
                &tiles::WorkplaceTile {
                    density: 1,
                    agents: vec![],
                }
                .into(),
            ),
        ),
    ]
}

fn build_detail_panel() -> impl druid::Widget<CurrentLeafState> {
    use druid::WidgetExt;
    druid::widget::Flex::column()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(druid::widget::Label::dynamic(
            |state: &CurrentLeafState, _env: &druid::Env| {
                use tiles::TileType;
                format!("Tile type: {}", state.leaf.tile.name())
            },
        ))
        .with_default_spacer()
        .with_child(druid::widget::Label::dynamic(
            |state: &CurrentLeafState, _env: &druid::Env| {
                format!("Address: {:?}", state.address.to_vec())
            },
        ))
        .with_default_spacer()
        .with_child(druid::widget::Label::dynamic(
            |state: &CurrentLeafState, _env: &druid::Env| {
                let (x, y) = state.address.to_xy();
                format!("Center: ({}, {})", x, y)
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
            |_ctx: &mut druid::EventCtx, state: &mut CurrentLeafState, _env: &druid::Env| {
                // NOTE: we need to do some juggling to adhere to borrowing rules
                let mut update = false;
                {
                    let mut engine = state.engine.lock().unwrap();
                    match engine.state.set_leaf_data(
                        *state.address,
                        &state.edited_data,
                        state::SerdeFormat::Toml,
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
                    *state = CurrentLeafState::new(*state.address, &engine, engine2);
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
                    |_ctx: &mut druid::EventCtx,
                     _state: &mut MetroLinesState,
                     _env: &druid::Env| {
                        // let mut engine = state.engine.lock().unwrap();
                        // // TODO: default metro speed specified here as 35 m/s, or 79 mph
                        // let id =
                        //     engine
                        //         .state
                        //         .add_metro_line(String::from("Metro Line"), None, 35, None);

                        // state.states.insert(id, MetroLineState::new());

                        // // TODO: this isn't quite enough to make it fully refresh
                        // ctx.children_changed();
                        // ctx.request_layout();
                        // ctx.request_paint();
                    },
                )
                .lens(State::metro_lines),
        )
        .with_default_spacer()
        .with_child(
            druid::widget::List::new(|| {
                druid::widget::Flex::row()
                    .with_child(
                        druid::widget::Checkbox::new("")
                            .lens(MetroLineState::visible)
                            .lens(MetroLineData::state),
                    )
                    .with_child(
                        druid::widget::Painter::new(|_ctx, _data: &MetroLineData, _env| {
                            // use druid::RenderContext;
                            // let metro_line = data.metro_line.lock().unwrap();
                            // let center = (ctx.size().width / 2.0, ctx.size().height / 2.0);
                            // let circle = druid::kurbo::Circle::new(
                            //     center,
                            //     f64::min(ctx.size().width, ctx.size().height) / 4.0,
                            // );
                            // let color = metro_line.color;
                            // ctx.fill(
                            //     circle,
                            //     &druid::Color::rgb8(color.red, color.green, color.blue),
                            // );
                        })
                        .fix_size(20.0, 20.0),
                    )
                    .with_child(
                        druid::widget::TextBox::new()
                            .lens(druid::lens::Map::new(
                                |data: &MetroLineData| {
                                    data.metro_line.lock().unwrap().data.name.clone()
                                },
                                |data: &mut MetroLineData, inner: String| {
                                    let mut metro_line = data.metro_line.lock().unwrap();
                                    metro_line.data.name = inner;
                                },
                            ))
                            .scroll()
                            .horizontal()
                            .disable_scrollbars()
                            .fix_width(200.0),
                    )
            })
            .scroll()
            .vertical()
            .fix_height(400.0)
            .lens(State::metro_lines),
        )
}

fn build_menu_panel() -> impl druid::Widget<State> {
    use druid::WidgetExt;

    druid::widget::Flex::column()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(druid::widget::Button::new("Save").on_click(
            |_ctx: &mut druid::EventCtx, state: &mut State, _env: &druid::Env| {
                let engine = state.engine.lock().unwrap();
                let timestamp = chrono::offset::Local::now();
                let path = format!(
                    "/tmp/metro_simulator_{}.json",
                    timestamp.format("%Y-%m-%d_%H-%M-%S"),
                );
                engine.dump_file(&std::path::PathBuf::from(&path)).unwrap();
                println!("Saved to {}", path);
            },
        ))
        .with_default_spacer()
        .with_child(druid::widget::Label::new("Fields:"))
        .with_default_spacer()
        .with_child(
            druid::widget::RadioGroup::new([
                ("None", FieldType::None),
                ("Population", FieldType::Population),
                ("Employment", FieldType::Employment),
                ("Land value", FieldType::LandValue),
            ])
            .lens(State::current_field),
        )
        .with_default_spacer()
        .with_child(druid::widget::Checkbox::new("Show qtree").lens(State::show_qtree))
        .with_default_spacer()
        .with_child(druid::widget::Checkbox::new("Show metros").lens(State::show_metros))
        .with_default_spacer()
        .with_child(druid::widget::Checkbox::new("Show metro keys").lens(State::show_metro_keys))
        .with_default_spacer()
        .with_child(
            druid::widget::Checkbox::new("Show metro directions")
                .lens(State::show_metro_directions),
        )
        .with_default_spacer()
        .with_child(druid::widget::Checkbox::new("Show highways").lens(State::show_highways))
        .with_default_spacer()
        .with_child(
            druid::widget::Checkbox::new("Show highway keys").lens(State::show_highway_keys),
        )
        .with_default_spacer()
        .with_child(
            druid::widget::Checkbox::new("Show highway directions")
                .lens(State::show_highway_directions),
        )
}

fn build_empty_panel() -> impl druid::Widget<()> {
    druid::widget::Flex::column()
}

#[derive(Debug, Clone, PartialEq, Eq, druid::Data)]
enum FieldType {
    None,
    Population,
    Employment,
    LandValue,
}

fn field_data_to_color(
    field_type: &FieldType,
    fields_state: &engine::FieldsState,
) -> Option<druid::Color> {
    let value = match field_type {
        FieldType::None => None,
        FieldType::Population => {
            let peak = 0.05;
            Some(f64::min(fields_state.population.people.density(), peak) / peak)
        }
        FieldType::Employment => {
            let peak = 0.05;
            Some(f64::min(fields_state.employment.workers.density(), peak) / peak)
        }
        FieldType::LandValue => None,
    };
    value.map(|val| druid::Color::hlca(120.0 * val, 80.0, 80.0, 0.5))
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct MetroLineState {
    visible: bool,
}

impl MetroLineState {
    fn new() -> Self {
        MetroLineState { visible: false }
    }
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct MetroLinesState {
    engine: Arc<Mutex<engine::Engine>>,
    states: druid::im::HashMap<metro::MetroLineHandle, MetroLineState>,
}

impl MetroLinesState {
    fn new(engine: Arc<Mutex<engine::Engine>>) -> Self {
        let mut states = druid::im::HashMap::new();

        {
            let engine = engine.lock().unwrap();
            for id in engine.state.metros.metro_lines().keys() {
                states.insert(*id, MetroLineState::new());
            }
        }

        Self { engine, states }
    }
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct MetroLineData {
    metro_line: Arc<Mutex<metro::MetroLine>>,
    state: MetroLineState,
}

impl MetroLineData {
    fn new(metro_line: &metro::MetroLine, state: &MetroLineState) -> Self {
        Self {
            metro_line: Arc::new(Mutex::new(metro_line.clone())),
            state: state.clone(),
        }
    }
}

impl druid::widget::ListIter<MetroLineData> for MetroLinesState {
    fn for_each(&self, mut cb: impl FnMut(&MetroLineData, usize)) {
        let engine = self.engine.lock().unwrap();

        for (i, (id, metro_line)) in engine.state.metros.metro_lines().iter().enumerate() {
            // TODO: this clone is disgusting
            let data = MetroLineData::new(metro_line, &self.states[id]);
            cb(&data, i);
        }
    }

    fn for_each_mut(&mut self, mut _cb: impl FnMut(&mut MetroLineData, usize)) {
        let mut _engine = self.engine.lock().unwrap();

        // for (i, (id, metro_line)) in engine.state.metros.metro_lines().iter_mut().enumerate() {
        //     // TODO: this double clone is even more disgusting
        //     let mut data = MetroLineData::new(metro_line, &self.states[id]);
        //     cb(&mut data, i);
        //     *metro_line = data.metro_line.lock().unwrap().clone();
        //     *self.states.get_mut(id).unwrap() = data.state;
        // }
    }

    fn data_len(&self) -> usize {
        self.engine.lock().unwrap().state.metros.metro_lines().len()
    }
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct CurrentLeafState {
    address: Rc<quadtree::Address>,
    leaf: Rc<state::LeafState<engine::FieldsState>>,
    tile_type: std::mem::Discriminant<tiles::Tile>,
    data: String,
    edited_data: String,

    engine: Arc<Mutex<engine::Engine>>,
}

impl CurrentLeafState {
    fn new(
        address: quadtree::Address,
        engine: &engine::Engine,
        engine_clone: Arc<Mutex<engine::Engine>>,
    ) -> Self {
        let leaf = engine.state.qtree.get_leaf(address).unwrap();
        let data = engine
            .state
            .get_leaf_data(address, state::SerdeFormat::Toml)
            .unwrap();
        Self {
            address: Rc::new(address),
            leaf: Rc::new(leaf.clone()),
            tile_type: std::mem::discriminant(&leaf.tile),
            data: data.clone(),
            edited_data: data,
            engine: engine_clone,
        }
    }
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct ContentState {
    scale: f64,
    tx: f64,
    ty: f64,

    min_scale: f64,
    max_scale: f64,

    mouse_pos: Option<druid::Point>,
}

impl ContentState {
    pub fn new(engine: Arc<Mutex<engine::Engine>>) -> Self {
        let (mut width, height) = DEFAULT_WINDOW_SIZE as (f64, f64);
        // account for the two side panels
        // TODO: make this less gross
        width -= 600.0;

        let min_dim = f64::min(width, height);
        let model_width = engine.lock().unwrap().state.qtree.width() as f64;

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
            mouse_pos: None,
        }
    }

    pub fn to_screen(&self, (x, y): (u64, u64)) -> (f64, f64) {
        self.to_screenf((x as f64, y as f64))
    }

    pub fn to_screenf(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (x * self.scale + self.tx, y * self.scale + self.ty)
    }

    pub fn to_model(&self, (x, y): (f64, f64)) -> (u64, u64) {
        (
            ((x - self.tx) / self.scale) as u64,
            ((y - self.ty) / self.scale) as u64,
        )
    }
}

struct Content {}

impl druid::Widget<State> for Content {
    fn event(
        &mut self,
        ctx: &mut druid::EventCtx,
        event: &druid::Event,
        state: &mut State,
        _env: &druid::Env,
    ) {
        let content = &mut state.content;
        use druid::Event::*;
        match event {
            MouseDown(mouse) | MouseUp(mouse) if mouse.buttons.has_left() => {
                content.mouse_pos = None;
            }
            MouseMove(mouse) if mouse.buttons.has_left() => {
                if let Some(old) = content.mouse_pos {
                    let diff = mouse.pos - old;
                    content.tx += diff.x;
                    content.ty += diff.y;
                    ctx.request_paint();
                }
                content.mouse_pos = Some(mouse.pos)
            }
            Wheel(mouse) => {
                let new_scale = f64::max(
                    f64::min(
                        content.scale * 1.1_f64.powf(-mouse.wheel_delta.y / 10.0),
                        content.max_scale,
                    ),
                    content.min_scale,
                );

                let mx = mouse.pos.x;
                let my = mouse.pos.y;

                // Zoom centered on mouse
                content.tx =
                    (mx * content.scale - mx * new_scale + content.tx * new_scale) / content.scale;
                content.ty =
                    (my * content.scale - my * new_scale + content.ty * new_scale) / content.scale;

                content.scale = new_scale;
                ctx.request_paint();
            }
            MouseDown(mouse) if mouse.buttons.has_right() => {
                let mut engine = state.engine.lock().unwrap();
                let (mx, my) = content.to_model(mouse.pos.into());
                let w = engine.state.qtree.width();
                if mx > 0 && mx < w && my > 0 && my < w {
                    let address = engine.state.qtree.get_address(mx, my).unwrap();
                    if address.depth() < engine.state.qtree.max_depth() as usize {
                        use state::{BranchState, LeafState};
                        engine
                            .state
                            .qtree
                            .split(
                                address,
                                BranchState::default(),
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
                let (mx, my) = content.to_model(mouse.pos.into());
                let w = engine.state.qtree.width();
                if mx > 0 && mx < w && my > 0 && my < w {
                    let address = engine.state.qtree.get_address(mx, my).unwrap();
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
        _ctx: &mut druid::LifeCycleCtx<'_, '_>,
        _event: &druid::LifeCycle,
        _state: &State,
        _env: &druid::Env,
    ) {
    }

    fn update(
        &mut self,
        ctx: &mut druid::UpdateCtx<'_, '_>,
        _old_data: &State,
        _state: &State,
        _env: &druid::Env,
    ) {
        ctx.request_paint();
    }

    fn layout(
        &mut self,
        _ctx: &mut druid::LayoutCtx<'_, '_>,
        bc: &druid::BoxConstraints,
        _state: &State,
        _env: &druid::Env,
    ) -> druid::Size {
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(DEFAULT_WINDOW_SIZE)
        }
    }

    fn paint(&mut self, ctx: &mut druid::PaintCtx, state: &State, env: &druid::Env) {
        use itertools::Itertools;

        let engine = state.engine.lock().unwrap();

        let (x1, y1) = state.content.to_model((0.0, 0.0));
        let (x2, y2) = state.content.to_model(ctx.size().into());

        let bounding_box = quadtree::Rect::corners(x1, y1, x2, y2);

        let mut qtree_visitor = PaintQtreeVisitor {
            ctx,
            env,
            state,
            visited: 0,
        };
        if state.show_qtree {
            engine
                .state
                .qtree
                .visit_rect(&mut qtree_visitor, &bounding_box)
                .unwrap();
        }

        // 5 pixel resolution
        let spline_scale = f64::max(5.0 / state.content.scale, 0.2);

        let qtree_visited = qtree_visitor.visited;
        let mut metro_total_visited = 0;

        if state.show_metros {
            for (_id, segment) in engine.state.railways.segments().iter().sorted() {
                let mut spline_visitor =
                    PaintSplineVisitor::new(ctx, env, state, state.show_metro_directions);
                segment
                    .visit_spline(&mut spline_visitor, spline_scale, &bounding_box)
                    .unwrap();
                metro_total_visited += &spline_visitor.visited;

                // TODO: show metro keys in new system
            }
        }

        let mut highway_total_visited = 0;

        if state.show_highways {
            for (_, highway_segment) in engine.state.highways.segments().iter().sorted() {
                let mut spline_visitor =
                    PaintSplineVisitor::new(ctx, env, state, state.show_highway_directions);
                highway_segment
                    .visit_spline(&mut spline_visitor, spline_scale, &bounding_box)
                    .unwrap();
                highway_total_visited += &spline_visitor.visited;

                if state.show_highway_keys {
                    let mut key_visitor = PaintHighwayKeysVisitor { ctx, env, state };

                    highway_segment
                        .visit_keys(&mut key_visitor, &bounding_box)
                        .unwrap();

                    // draw start and end
                    let keys = highway_segment.keys();
                    if let (Some(first), Some(last)) = (keys.first(), keys.last()) {
                        use druid::RenderContext;

                        ctx.fill(
                            druid::kurbo::Circle::new(
                                state.content.to_screenf((first.x, first.y)),
                                4.0,
                            ),
                            &druid::Color::grey8(255),
                        );
                        ctx.fill(
                            druid::kurbo::Circle::new(
                                state.content.to_screenf((last.x, last.y)),
                                4.0,
                            ),
                            &druid::Color::grey8(255),
                        );
                    }
                }
            }
        }

        println!(
            "qtree: {}, metros: {}, highways: {}",
            qtree_visited, metro_total_visited, highway_total_visited
        );
    }
}

struct PaintQtreeVisitor<'a, 'b, 'c, 'd, 'e, 'f> {
    ctx: &'a mut druid::PaintCtx<'c, 'd, 'e>,
    #[allow(dead_code)]
    env: &'b druid::Env,
    state: &'f State,

    visited: u64,
}

impl<'a, 'b, 'c, 'd, 'e, 'f> PaintQtreeVisitor<'a, 'b, 'c, 'd, 'e, 'f> {
    fn get_rect(&self, data: &quadtree::VisitData) -> druid::Rect {
        let width = data.width as f64 * self.state.content.scale;
        druid::Rect::from_origin_size(
            self.state.content.to_screen((data.x, data.y)),
            (width, width),
        )
    }

    fn get_full_rect(&self, data: &quadtree::VisitData) -> druid::Rect {
        let width = data.width as f64 * self.state.content.scale + 1.0;
        druid::Rect::from_origin_size(
            self.state.content.to_screen((data.x, data.y)),
            (width, width),
        )
    }

    fn maybe_draw_field(
        &mut self,
        fields: &engine::FieldsState,
        data: &quadtree::VisitData,
        is_leaf: bool,
    ) {
        let width = data.width as f64 * self.state.content.scale;
        let threshold = 10.0;
        if is_leaf || width >= threshold && width < threshold * 2.0 {
            if let Some(color) = field_data_to_color(&self.state.current_field, fields) {
                use druid::RenderContext;

                // TODO: get_full_rect looks weird because there's overlap,
                // get_rect looks weird because there's a gap.
                let rect = self.get_rect(data);
                self.ctx.fill(rect, &color);
            }
        }
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f>
    quadtree::Visitor<
        state::BranchState<engine::FieldsState>,
        state::LeafState<engine::FieldsState>,
        anyhow::Error,
    > for PaintQtreeVisitor<'a, 'b, 'c, 'd, 'e, 'f>
{
    fn visit_branch_pre(
        &mut self,
        _branch: &state::BranchState<engine::FieldsState>,
        data: &quadtree::VisitData,
    ) -> anyhow::Result<bool> {
        let should_descend = data.width as f64 * self.state.content.scale >= 5.0;

        if !should_descend {
            use druid::RenderContext;

            // draw a rectangle to indicate that there's stuff here
            let full_rect = self.get_full_rect(data);
            self.ctx.fill(full_rect, &druid::Color::grey8(100));
        }

        Ok(should_descend)
    }

    fn visit_leaf(
        &mut self,
        leaf: &state::LeafState<engine::FieldsState>,
        data: &quadtree::VisitData,
    ) -> anyhow::Result<()> {
        use druid::RenderContext;

        let width = data.width as f64 * self.state.content.scale;
        let rect = self.get_rect(data);
        let full_rect = self.get_full_rect(data);

        if let Some(current_leaf) = &self.state.current_leaf {
            if *current_leaf.address == data.address {
                self.ctx.stroke(rect, &druid::Color::grey8(200), 5.0);
            }
        }

        use tiles::Tile::*;
        match &leaf.tile {
            WaterTile(tiles::WaterTile {}) => {
                self.ctx.fill(full_rect, &druid::Color::rgb8(0, 0, 150));
            }
            HousingTile(tiles::HousingTile { .. }) => {
                let circle = druid::kurbo::Circle::new(rect.center(), width / 8.0);
                self.ctx.fill(circle, &druid::Color::grey8(255));
            }
            WorkplaceTile(tiles::WorkplaceTile { .. }) => {
                let triangle = triangle(
                    rect.center().into(),
                    width / 6.0,
                    -std::f64::consts::FRAC_PI_2,
                );
                self.ctx.fill(&triangle[..], &druid::Color::grey8(255));
            }
            MetroStationTile(tiles::MetroStationTile { .. }) => {
                let circle = druid::kurbo::Circle::new(rect.center(), width / 4.0);
                self.ctx.stroke(circle, &druid::Color::grey8(255), 1.0);
            }
            _ => (),
        }

        self.maybe_draw_field(&leaf.fields, data, true);

        self.visited += 1;

        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &state::BranchState<engine::FieldsState>,
        data: &quadtree::VisitData,
    ) -> anyhow::Result<()> {
        self.maybe_draw_field(&branch.fields, data, false);
        Ok(())
    }
}

fn triangle((x, y): (f64, f64), radius: f64, theta: f64) -> [druid::kurbo::PathEl; 4] {
    use druid::kurbo::PathEl::*;
    use std::f64::consts::PI;
    let sides = 3;
    let mut points = [(0.0, 0.0); 3];
    for (i, point) in points.iter_mut().enumerate() {
        let t = (PI * 2.0) / sides as f64 * i as f64 + theta;
        *point = (x + t.cos() * radius, y + t.sin() * radius)
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
    #[allow(dead_code)]
    env: &'b druid::Env,
    state: &'f State,

    visited: u64,

    draw_arrows: bool,
    last_arrow: Option<(f64, f64)>,
}

impl<'a, 'b, 'c, 'd, 'e, 'f> PaintSplineVisitor<'a, 'b, 'c, 'd, 'e, 'f> {
    fn new(
        ctx: &'a mut druid::PaintCtx<'c, 'd, 'e>,
        env: &'b druid::Env,
        state: &'f State,
        draw_arrows: bool,
    ) -> Self {
        Self {
            ctx,
            env,
            state,
            visited: 0,
            draw_arrows,
            last_arrow: None,
        }
    }

    fn visit(
        &mut self,
        color: &druid::Color,
        line_width: f64,
        vertex: cgmath::Vector2<f64>,
        _t: f64,
        prev: Option<cgmath::Vector2<f64>>,
    ) -> Result<(), anyhow::Error> {
        use druid::RenderContext;

        let point = (
            vertex.x * self.state.content.scale + self.state.content.tx,
            vertex.y * self.state.content.scale + self.state.content.ty,
        );

        if let Some(prev) = prev {
            let last_point = (
                prev.x * self.state.content.scale + self.state.content.tx,
                prev.y * self.state.content.scale + self.state.content.ty,
            );

            self.ctx.stroke(
                druid::kurbo::Line::new(last_point, point),
                color,
                line_width,
            );

            if self.draw_arrows && last_point != point {
                let (x1, y1) = last_point;
                let (x2, y2) = point;
                let center = ((x2 + x1) / 2.0, (y2 + y1) / 2.0);

                if match self.last_arrow {
                    Some(last) => {
                        let dist_sq = (center.0 - last.0).powi(2) + (center.1 - last.1).powi(2);
                        // space arrows at least 30 pixels apart
                        dist_sq >= 30.0_f64.powi(2)
                    }
                    None => true,
                } {
                    let theta = (y2 - y1).atan2(x2 - x1);
                    self.ctx
                        .fill(&triangle(center, 5.0, theta)[..], &druid::Color::grey8(255));
                    self.last_arrow = Some(center);
                }
            }
        };

        self.visited += 1;

        Ok(())
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f>
    spline_util::SplineVisitor<
        network::Segment<metro::RailwaySegment>,
        cgmath::Vector2<f64>,
        anyhow::Error,
    > for PaintSplineVisitor<'a, 'b, 'c, 'd, 'e, 'f>
{
    fn visit(
        &mut self,
        _segment: &network::Segment<metro::RailwaySegment>,
        vertex: cgmath::Vector2<f64>,
        t: f64,
        prev: Option<cgmath::Vector2<f64>>,
    ) -> Result<(), anyhow::Error> {
        // TODO: display metro line colors with new system
        let color = druid::Color::grey8(255);
        self.visit(&color, 2.0, vertex, t, prev)
    }
}

impl<'a, 'b, 'c, 'd, 'e, 'f>
    spline_util::SplineVisitor<
        network::Segment<highway::HighwaySegment>,
        cgmath::Vector2<f64>,
        anyhow::Error,
    > for PaintSplineVisitor<'a, 'b, 'c, 'd, 'e, 'f>
{
    fn visit(
        &mut self,
        _segment: &network::Segment<highway::HighwaySegment>,
        vertex: cgmath::Vector2<f64>,
        t: f64,
        prev: Option<cgmath::Vector2<f64>>,
    ) -> Result<(), anyhow::Error> {
        self.visit(&druid::Color::grey8(204), 1.0, vertex, t, prev)
    }
}

struct PaintHighwayKeysVisitor<'a, 'b, 'c, 'd, 'e, 'f> {
    ctx: &'a mut druid::PaintCtx<'c, 'd, 'e>,
    #[allow(dead_code)]
    env: &'b druid::Env,
    state: &'f State,
}

impl<'a, 'b, 'c, 'd, 'e, 'f> network::KeyVisitor<highway::HighwaySegment, anyhow::Error>
    for PaintHighwayKeysVisitor<'a, 'b, 'c, 'd, 'e, 'f>
{
    fn visit(
        &mut self,
        _segment: &network::Segment<highway::HighwaySegment>,
        key: &network::Key,
    ) -> Result<(), anyhow::Error> {
        use druid::RenderContext;

        let point = (
            key.x * self.state.content.scale + self.state.content.tx,
            key.y * self.state.content.scale + self.state.content.ty,
        );
        self.ctx.fill(
            druid::kurbo::Circle::new(point, 2.0),
            &druid::Color::grey8(255),
        );

        Ok(())
    }
}
