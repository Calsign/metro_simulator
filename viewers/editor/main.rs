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

    let engine = match args.load {
        Some(path) => engine::state::State::load_file(&path).unwrap(),
        None => engine::state::State::new(
            engine::config::Config::load_file(&std::path::PathBuf::from(DEFAULT_CONFIG)).unwrap(),
        ),
    };

    let state = State {
        content: ContentState::new(engine),
    };

    druid::AppLauncher::with_window(window)
        .launch(state)
        .unwrap();
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct State {
    content: ContentState,
}

fn build_root_widget() -> impl druid::Widget<State> {
    use druid::WidgetExt;
    druid::widget::Flex::row()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(
            druid::widget::Flex::column()
                .with_child(
                    druid::widget::Maybe::new(build_detail_panel, build_empty_panel)
                        .lens(ContentState::current_leaf)
                        .lens(State::content),
                )
                .expand()
                .padding((20.0, 20.0))
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
                    let engine1 = state.engine.clone();
                    let engine2 = state.engine.clone();
                    let engine = engine1.lock().unwrap();
                    *state = CurrentLeafState::new((*state.address).clone(), &engine, engine2);
                }
            },
        ))
}

fn build_menu_panel() -> impl druid::Widget<State> {
    druid::widget::Flex::column()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(druid::widget::Button::new("Save").on_click(
            |ctx: &mut druid::EventCtx, state: &mut State, env: &druid::Env| {
                let engine = state.content.engine.lock().unwrap();
                let timestamp = chrono::offset::Local::now();
                let path = format!(
                    "/tmp/metro_simulator_{}",
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
struct CurrentLeafState {
    address: Rc<quadtree::Address>,
    leaf: Rc<engine::state::LeafState>,
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
    pub fn new(engine: engine::state::State) -> Self {
        let (mut width, height) = DEFAULT_WINDOW_SIZE as (f64, f64);
        // account for the two side panels
        // TODO: make this less gross
        width -= 600.0;

        let min_dim = f64::min(width, height);
        let model_width = engine.qtree.width() as f64;

        let scale = min_dim / model_width / 2.0;
        let tx = width / 2.0 - model_width * scale / 2.0;
        let ty = height / 2.0 - model_width * scale / 2.0;

        let min_scale = min_dim / model_width / 2.0;
        let max_scale = 100.0;

        Self {
            engine: Arc::new(Mutex::new(engine)),
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

        let mut visitor = PaintVisitor { ctx, env, state };
        engine
            .qtree
            .visit_rect(&mut visitor, &quadtree::Rect::corners(x1, y1, x2, y2))
            .unwrap();
    }
}

struct PaintVisitor<'a, 'b, 'c, 'd, 'e, 'f> {
    ctx: &'a mut druid::PaintCtx<'c, 'd, 'e>,
    env: &'b druid::Env,
    state: &'f ContentState,
}

impl<'a, 'b, 'c, 'd, 'e, 'f>
    quadtree::Visitor<engine::state::BranchState, engine::state::LeafState, anyhow::Error>
    for PaintVisitor<'a, 'b, 'c, 'd, 'e, 'f>
{
    fn visit_branch(
        &mut self,
        branch: &engine::state::BranchState,
        data: &quadtree::VisitData,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    fn visit_leaf(
        &mut self,
        leaf: &engine::state::LeafState,
        data: &quadtree::VisitData,
    ) -> anyhow::Result<()> {
        use druid::RenderContext;
        let rect = druid::Rect::from_origin_size(
            self.state.to_screen((data.x, data.y)),
            (
                data.width as f64 * self.state.scale,
                data.width as f64 * self.state.scale,
            ),
        );
        let color = druid::Color::grey8(200);
        let mut width = 1.0;
        if let Some(current_leaf) = &self.state.current_leaf {
            if *current_leaf.address == data.address {
                width = 5.0;
            }
        }
        self.ctx.stroke(rect, &color, width);
        Ok(())
    }
}
