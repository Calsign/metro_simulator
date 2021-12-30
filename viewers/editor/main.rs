use std::rc::Rc;
use std::sync::{Arc, Mutex};

static DEFAULT_WINDOW_SIZE: (f64, f64) = (1920.0, 1080.0);
static WINDOW_TITLE: &str = "Metro Simulator";
static DEFAULT_CONFIG: &str = "config/debug.toml";

fn main() {
    let window = druid::WindowDesc::new(build_root_widget)
        .title(WINDOW_TITLE)
        .window_size(DEFAULT_WINDOW_SIZE);

    let state = State {
        content: ContentState::new(engine::state::State::new(
            engine::config::Config::load_file(&std::path::PathBuf::from(DEFAULT_CONFIG)).unwrap(),
        )),
        label: "foobar".into(),
    };

    druid::AppLauncher::with_window(window)
        .launch(state)
        .unwrap();
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct State {
    content: ContentState,
    label: String,
}

fn build_root_widget() -> impl druid::Widget<State> {
    use druid::WidgetExt;
    druid::widget::Flex::row()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(
            druid::widget::Flex::column()
                .with_child(
                    druid::widget::ViewSwitcher::new(
                        |state: &Option<CurrentLeafState>, env: &druid::Env| state.clone(),
                        |selector: &Option<CurrentLeafState>, _state, _env| {
                            Box::new(match selector {
                                Some(state) => build_detail_panel(),
                                None => druid::widget::Flex::row().into(),
                            })
                        },
                    )
                    .lens(ContentState::current_leaf)
                    .lens(State::content),
                )
                .padding((20.0, 20.0)),
        )
        .with_child(Content {}.lens(State::content))
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
        .with_child(druid::widget::TextBox::multiline().lens(CurrentLeafState::json))
}

#[derive(Debug, Clone, druid::Data, druid::Lens)]
struct CurrentLeafState {
    address: Rc<quadtree::Address>,
    leaf: Rc<engine::state::LeafState>,
    json: String,
    edited_json: String,
}

impl CurrentLeafState {
    fn new(address: quadtree::Address, engine: &engine::state::State) -> Self {
        let leaf = engine.qtree.get_leaf(address.clone()).unwrap();
        let json = engine.get_leaf_json(address.clone()).unwrap();
        Self {
            address: Rc::new(address),
            leaf: Rc::new(leaf.clone()),
            json: json.clone(),
            edited_json: json.clone(),
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
        let (width, height) = DEFAULT_WINDOW_SIZE as (f64, f64);

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
                                address,
                                BranchState {},
                                quadtree::QuadMap::new(
                                    LeafState::default(),
                                    LeafState::default(),
                                    LeafState::default(),
                                    LeafState::default(),
                                ),
                            )
                            .unwrap();
                        state.current_leaf = None;
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
                    state.current_leaf = Some(CurrentLeafState::new(address, &engine));
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
