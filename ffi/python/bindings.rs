use pyo3::prelude::*;

pyo3::create_exception!(engine, PyEngineError, pyo3::exceptions::PyException);

#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("State error: {0}")]
    StateError(#[from] state::Error),
    #[error("Config error: {0}")]
    ConfigError(#[from] state::ConfigError),
    #[error("Engine error: {0}")]
    EngineError(#[from] engine::Error),
    #[error("Quadtree error: {0}")]
    QuadtreeError(#[from] quadtree::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl std::convert::From<EngineError> for PyErr {
    fn from(err: EngineError) -> PyErr {
        return PyEngineError::new_err(err.to_string());
    }
}

fn wrap_err<T, I: Into<EngineError>>(result: Result<T, I>) -> PyResult<T> {
    return match result {
        Ok(t) => Ok(t),
        Err(e) => Err(PyErr::from(e.into())),
    };
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct Address {
    address: quadtree::Address,
}

#[pymethods]
impl Address {
    #[new]
    fn new(address: Vec<u8>, max_depth: u32) -> PyResult<Self> {
        match quadtree::Address::try_from(&address, max_depth) {
            Some(address) => Ok(address.into()),
            None => Err(pyo3::exceptions::PyValueError::new_err(
                "quadrants must be in [0, 3]",
            )),
        }
    }

    fn get(&self) -> Vec<u8> {
        self.address.clone().into()
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct Config {
    config: state::Config,
}

#[pymethods]
impl Config {
    #[new]
    fn new(path: std::path::PathBuf) -> PyResult<Self> {
        Ok(Config {
            config: wrap_err(state::Config::load_file(&path))?,
        })
    }

    #[staticmethod]
    fn from_json(json: String) -> PyResult<Self> {
        let config: state::Config = wrap_err(serde_json::from_str(&json))?;
        Ok(config.into())
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct BranchState {
    branch: state::BranchState<engine::FieldsState>,
}

#[pymethods]
impl BranchState {
    #[new]
    fn new() -> Self {
        state::BranchState::default().into()
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct LeafState {
    leaf: state::LeafState<engine::FieldsState>,
}

#[pymethods]
impl LeafState {
    #[new]
    fn new() -> Self {
        state::LeafState::default().into()
    }

    #[staticmethod]
    fn from_json(json: String) -> PyResult<Self> {
        let leaf: state::LeafState<engine::FieldsState> = wrap_err(serde_json::from_str(&json))?;
        Ok(leaf.into())
    }

    #[getter]
    fn name(&self) -> &'static str {
        use tiles::TileType;
        self.leaf.tile.name()
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct Engine {
    engine: engine::Engine,
}

#[pymethods]
impl Engine {
    #[new]
    fn new(config: &Config) -> Self {
        Self {
            engine: engine::Engine::new(config.config.clone()),
        }
    }

    #[staticmethod]
    fn load(path: std::path::PathBuf) -> PyResult<Self> {
        Ok(Self {
            engine: wrap_err(engine::Engine::load_file(&path))?,
        })
    }

    fn save(&self, path: std::path::PathBuf) -> PyResult<()> {
        wrap_err(self.engine.dump_file(&path))
    }

    #[getter]
    fn width(&self) -> u64 {
        self.engine.state.qtree.width()
    }

    #[getter]
    fn max_depth(&self) -> u32 {
        self.engine.state.qtree.max_depth()
    }

    fn visit(&self, branch_visitor: &PyAny, leaf_visitor: &PyAny) -> PyResult<()> {
        if !branch_visitor.is_callable() || !leaf_visitor.is_callable() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "visitors must be callable",
            ));
        }
        let mut visitor = PyQtreeVisitor {
            branch_visitor: branch_visitor.into(),
            leaf_visitor: leaf_visitor.into(),
        };
        self.engine.state.qtree.visit(&mut visitor)
    }

    fn visit_rect(
        &self,
        branch_visitor: &PyAny,
        leaf_visitor: &PyAny,
        min_x: u64,
        max_x: u64,
        min_y: u64,
        max_y: u64,
    ) -> PyResult<()> {
        if !branch_visitor.is_callable() || !leaf_visitor.is_callable() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "visitors must be callable",
            ));
        }
        let mut visitor = PyQtreeVisitor {
            branch_visitor: branch_visitor.into(),
            leaf_visitor: leaf_visitor.into(),
        };
        self.engine.state.qtree.visit_rect(
            &mut visitor,
            &quadtree::Rect {
                min_x,
                max_x,
                min_y,
                max_y,
            },
        )
    }

    fn add_railway_junction(
        &mut self,
        x: f64,
        y: f64,
        data: &RailwayJunctionData,
    ) -> RailwayJunctionHandle {
        RailwayJunctionHandle {
            handle: self
                .engine
                .state
                .railways
                .add_junction((x, y), data.data.clone()),
        }
    }

    fn add_railway_segment(
        &mut self,
        data: &RailwaySegmentData,
        start: &RailwayJunctionHandle,
        end: &RailwayJunctionHandle,
        keys: Option<Vec<(f64, f64)>>,
    ) -> RailwaySegmentHandle {
        RailwaySegmentHandle {
            handle: self.engine.state.railways.add_segment(
                data.data.clone(),
                start.handle,
                end.handle,
                keys.map(|ks| {
                    ks.iter()
                        .map(|(x, y)| cgmath::Vector2 { x: *x, y: *y })
                        .collect()
                }),
            ),
        }
    }

    fn add_metro_line(
        &mut self,
        data: &MetroLineData,
        segments: Vec<pyo3::PyRef<RailwaySegmentHandle>>,
    ) -> MetroLineHandle {
        MetroLineHandle {
            handle: self.engine.state.metros.add_metro_line(
                data.data.clone(),
                segments.iter().map(|segment| segment.handle).collect(),
                &self.engine.state.railways,
            ),
        }
    }

    fn add_highway_junction(
        &mut self,
        x: f64,
        y: f64,
        data: &HighwayJunctionData,
    ) -> HighwayJunctionHandle {
        HighwayJunctionHandle {
            handle: self
                .engine
                .state
                .highways
                .add_junction((x, y), data.data.clone()),
        }
    }

    fn add_highway_segment(
        &mut self,
        data: &HighwaySegmentData,
        start: &HighwayJunctionHandle,
        end: &HighwayJunctionHandle,
        keys: Option<Vec<(f64, f64)>>,
    ) -> HighwaySegmentHandle {
        HighwaySegmentHandle {
            handle: self.engine.state.highways.add_segment(
                data.data.clone(),
                start.handle,
                end.handle,
                keys.map(|ks| {
                    ks.iter()
                        .map(|(x, y)| cgmath::Vector2 { x: *x, y: *y })
                        .collect()
                }),
            ),
        }
    }

    fn add_agent(
        &mut self,
        data: &AgentData,
        housing: &Address,
        workplace: Option<&Address>,
    ) -> u64 {
        self.engine.add_agent(
            data.data.clone(),
            housing.address,
            workplace.map(|a| a.address),
        )
    }

    fn validate_highways(&self) {
        self.engine.state.highways.validate();
    }

    fn validate_metro_lines(&self) {
        self.engine
            .state
            .metros
            .validate(&self.engine.state.railways);
    }

    fn get_address(&self, x: u64, y: u64) -> PyResult<Address> {
        Ok(wrap_err(self.engine.state.qtree.get_address(x, y))?.into())
    }

    fn split(
        &mut self,
        address: &Address,
        data: &BranchState,
        nw: &LeafState,
        ne: &LeafState,
        sw: &LeafState,
        se: &LeafState,
    ) -> PyResult<()> {
        wrap_err(self.engine.state.qtree.split(
            address.address.clone(),
            data.branch.clone(),
            quadtree::QuadMap::new(
                nw.leaf.clone(),
                ne.leaf.clone(),
                sw.leaf.clone(),
                se.leaf.clone(),
            ),
        ))
    }

    fn get_leaf_json(&self, address: &Address) -> PyResult<String> {
        wrap_err(
            self.engine
                .state
                .get_leaf_data(address.address.clone(), state::SerdeFormat::Json),
        )
    }

    fn set_leaf_json(&mut self, address: &Address, json: &str) -> PyResult<()> {
        wrap_err(self.engine.state.set_leaf_data(
            address.address.clone(),
            json,
            state::SerdeFormat::Json,
        ))
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct VisitData {
    data: quadtree::VisitData,
}

#[pymethods]
impl VisitData {
    #[getter]
    fn address(&self) -> Address {
        self.data.address.clone().into()
    }

    #[getter]
    fn depth(&self) -> u32 {
        self.data.depth
    }

    #[getter]
    fn x(&self) -> u64 {
        self.data.x
    }

    #[getter]
    fn y(&self) -> u64 {
        self.data.y
    }

    #[getter]
    fn width(&self) -> u64 {
        self.data.width
    }
}

struct PyQtreeVisitor {
    branch_visitor: PyObject,
    leaf_visitor: PyObject,
}

impl
    quadtree::Visitor<
        state::BranchState<engine::FieldsState>,
        state::LeafState<engine::FieldsState>,
        PyErr,
    > for PyQtreeVisitor
{
    fn visit_branch_pre(
        &mut self,
        branch: &state::BranchState<engine::FieldsState>,
        data: &quadtree::VisitData,
    ) -> PyResult<bool> {
        Python::with_gil(|py| {
            let branch = BranchState::from(branch.clone());
            let data = VisitData::from(data.clone());
            Ok(self.branch_visitor.call1(py, (branch, data))?.is_true(py)?)
        })
    }

    fn visit_leaf(
        &mut self,
        leaf: &state::LeafState<engine::FieldsState>,
        data: &quadtree::VisitData,
    ) -> PyResult<()> {
        Python::with_gil(|py| {
            let leaf = LeafState::from(leaf.clone());
            let data = VisitData::from(data.clone());
            self.leaf_visitor.call1(py, (leaf, data))?;
            Ok(())
        })
    }

    fn visit_branch_post(
        &mut self,
        _branch: &state::BranchState<engine::FieldsState>,
        _data: &quadtree::VisitData,
    ) -> PyResult<()> {
        Ok(())
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct Station {
    station: metro::Station,
}

#[pymethods]
impl Station {
    #[new]
    fn new(name: &str, address: &Address) -> Self {
        Self {
            station: metro::Station {
                name: name.to_string(),
                address: address.address.clone(),
            },
        }
    }

    #[getter]
    fn address(&self) -> Address {
        self.station.address.clone().into()
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct RailwaySegmentData {
    data: metro::RailwaySegment,
}

#[pymethods]
impl RailwaySegmentData {
    #[new]
    fn new(speed_limit: Option<u32>) -> Self {
        Self {
            data: metro::RailwaySegment::new(speed_limit),
        }
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct RailwayJunctionData {
    data: metro::RailwayJunction,
}

#[pymethods]
impl RailwayJunctionData {
    #[new]
    fn new(station: Option<&Station>) -> Self {
        Self {
            data: metro::RailwayJunction::new(station.map(|station| station.station.clone())),
        }
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct RailwayJunctionHandle {
    handle: network::JunctionHandle,
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct RailwaySegmentHandle {
    handle: network::SegmentHandle,
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct Schedule {
    schedule: metro::Schedule,
}

#[pymethods]
impl Schedule {
    #[staticmethod]
    fn fixed_frequency(fixed_frequency: u64) -> Self {
        Self {
            schedule: metro::Schedule::fixed_frequency(fixed_frequency),
        }
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct MetroLineData {
    data: metro::MetroLineData,
}

#[pymethods]
impl MetroLineData {
    #[new]
    fn new(color: (u8, u8, u8), name: String, schedule: &Schedule, speed_limit: u32) -> Self {
        Self {
            data: metro::MetroLineData {
                color: color.into(),
                name,
                schedule: schedule.schedule.clone(),
                speed_limit,
            },
        }
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct MetroLineHandle {
    handle: metro::MetroLineHandle,
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct HighwaySegmentData {
    data: highway::HighwaySegment,
}

#[pymethods]
impl HighwaySegmentData {
    #[new]
    fn new(
        name: Option<String>,
        refs: Vec<String>,
        lanes: Option<u32>,
        speed_limit: Option<u32>,
    ) -> Self {
        Self {
            data: highway::HighwaySegment::new(name, refs, lanes, speed_limit),
        }
    }
}

#[pyclass]
#[derive(Clone, Copy)]
struct RampDirection {
    direction: highway::RampDirection,
}

#[pymethods]
impl RampDirection {
    #[staticmethod]
    fn on_ramp() -> Self {
        Self {
            direction: highway::RampDirection::OnRamp,
        }
    }

    #[staticmethod]
    fn off_ramp() -> Self {
        Self {
            direction: highway::RampDirection::OffRamp,
        }
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct HighwayJunctionData {
    data: highway::HighwayJunction,
}

#[pymethods]
impl HighwayJunctionData {
    #[new]
    fn new(ramp_direction: Option<RampDirection>) -> Self {
        Self {
            data: highway::HighwayJunction::new(ramp_direction.map(|r| r.direction)),
        }
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct HighwayJunctionHandle {
    handle: network::JunctionHandle,
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct HighwaySegmentHandle {
    handle: network::SegmentHandle,
}

#[pyclass]
#[derive(Clone, Copy)]
struct Date {
    date: chrono::NaiveDate,
}

#[pymethods]
impl Date {
    #[staticmethod]
    fn from_ymd(year: i32, month: u32, day: u32) -> Self {
        Self {
            date: chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap(),
        }
    }
}

#[pyclass]
#[derive(Clone)]
struct AgentData {
    data: agent::AgentData,
}

#[pymethods]
impl AgentData {
    #[new]
    fn new(birthday: Date, years_of_education: u32) -> Self {
        Self {
            data: agent::AgentData {
                birthday: birthday.date,
                years_of_education,
            },
        }
    }
}

#[pyfunction]
fn min_creation_time() -> i64 {
    i64::MIN
}

#[pymodule]
fn engine(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Config>()?;
    m.add_class::<Address>()?;
    m.add_class::<BranchState>()?;
    m.add_class::<LeafState>()?;
    m.add_class::<VisitData>()?;
    m.add_class::<Engine>()?;

    m.add_class::<RailwaySegmentData>()?;
    m.add_class::<RailwayJunctionData>()?;
    m.add_class::<Station>()?;
    m.add_class::<RailwayJunctionHandle>()?;
    m.add_class::<RailwaySegmentHandle>()?;
    m.add_class::<MetroLineData>()?;
    m.add_class::<Schedule>()?;
    m.add_class::<MetroLineHandle>()?;

    m.add_class::<HighwaySegmentData>()?;
    m.add_class::<HighwayJunctionData>()?;
    m.add_class::<RampDirection>()?;
    m.add_class::<HighwayJunctionHandle>()?;
    m.add_class::<HighwaySegmentHandle>()?;

    m.add_class::<Date>()?;
    m.add_class::<AgentData>()?;

    m.add_function(wrap_pyfunction!(min_creation_time, m)?)?;

    return Ok(());
}
