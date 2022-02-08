use pyo3::prelude::*;

pyo3::create_exception!(engine, PyEngineError, pyo3::exceptions::PyException);

#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("Config error: {0}")]
    ConfigError(#[from] engine::config::Error),
    #[error("State error: {0}")]
    StateError(#[from] engine::state::Error),
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
    fn new(address: Vec<u8>) -> PyResult<Self> {
        match quadtree::Address::try_from(&address) {
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
    config: engine::config::Config,
}

#[pymethods]
impl Config {
    #[new]
    fn new(path: std::path::PathBuf) -> PyResult<Self> {
        Ok(Config {
            config: wrap_err(engine::config::Config::load_file(&path))?,
        })
    }

    #[staticmethod]
    fn from_json(json: String) -> PyResult<Self> {
        let config: engine::config::Config = wrap_err(serde_json::from_str(&json))?;
        Ok(config.into())
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct BranchState {
    branch: engine::state::BranchState,
}

#[pymethods]
impl BranchState {
    #[new]
    fn new() -> Self {
        engine::state::BranchState::default().into()
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct LeafState {
    leaf: engine::state::LeafState,
}

#[pymethods]
impl LeafState {
    #[new]
    fn new() -> Self {
        engine::state::LeafState::default().into()
    }

    #[staticmethod]
    fn from_json(json: String) -> PyResult<Self> {
        let leaf: engine::state::LeafState = wrap_err(serde_json::from_str(&json))?;
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
struct State {
    state: engine::state::State,
}

#[pymethods]
impl State {
    #[new]
    fn new(config: &Config) -> Self {
        State {
            state: engine::state::State::new(config.config.clone()),
        }
    }

    #[staticmethod]
    fn load(path: std::path::PathBuf) -> PyResult<Self> {
        Ok(State {
            state: wrap_err(engine::state::State::load_file(&path))?,
        })
    }

    fn save(&self, path: std::path::PathBuf) -> PyResult<()> {
        wrap_err(self.state.dump_file(&path))
    }

    #[getter]
    fn width(&self) -> u64 {
        self.state.qtree.width()
    }

    #[getter]
    fn max_depth(&self) -> usize {
        self.state.qtree.max_depth()
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
        self.state.qtree.visit(&mut visitor)
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
        self.state.qtree.visit_rect(
            &mut visitor,
            &quadtree::Rect {
                min_x,
                max_x,
                min_y,
                max_y,
            },
        )
    }

    fn add_metro_line(
        &mut self,
        name: String,
        color: Option<(u8, u8, u8)>,
        keys: Option<Vec<pyo3::PyRef<MetroKey>>>,
    ) -> PyResult<()> {
        self.state.add_metro_line(
            name,
            color.map(|c| c.into()),
            keys.map(|v| v.iter().map(|k| k.key.clone()).collect()),
        );
        Ok(())
    }

    fn get_metro_line(&self, id: u64) -> Option<MetroLine> {
        self.state
            .metro_lines
            .get(&id)
            .map(|line| line.clone().into())
    }

    fn visit_metro_line(
        &self,
        metro_line: &MetroLine,
        visitor: &PyAny,
        step: f64,
        min_x: u64,
        max_x: u64,
        min_y: u64,
        max_y: u64,
    ) -> PyResult<()> {
        if !visitor.is_callable() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "visitor must be callable",
            ));
        }
        let mut visitor = PySplineVisitor {
            visitor: visitor.into(),
        };
        metro_line.metro_line.visit_spline(
            &mut visitor,
            step,
            &quadtree::Rect {
                min_x,
                max_x,
                min_y,
                max_y,
            },
        )
    }

    fn get_address(&self, x: u64, y: u64) -> PyResult<Address> {
        Ok(wrap_err(self.state.qtree.get_address(x, y))?.into())
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
        wrap_err(self.state.qtree.split(
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
            self.state
                .get_leaf_data(address.address.clone(), engine::state::SerdeFormat::Json),
        )
    }

    fn set_leaf_json(&mut self, address: &Address, json: &str) -> PyResult<()> {
        wrap_err(self.state.set_leaf_data(
            address.address.clone(),
            json,
            engine::state::SerdeFormat::Json,
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
    fn depth(&self) -> usize {
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

impl quadtree::Visitor<engine::state::BranchState, engine::state::LeafState, PyErr>
    for PyQtreeVisitor
{
    fn visit_branch_pre(
        &mut self,
        branch: &engine::state::BranchState,
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
        leaf: &engine::state::LeafState,
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
        branch: &engine::state::BranchState,
        data: &quadtree::VisitData,
    ) -> PyResult<()> {
        Ok(())
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct MetroStation {
    station: metro::Station,
}

#[pymethods]
impl MetroStation {
    #[new]
    fn new(address: &Address) -> Self {
        Self {
            station: metro::Station {
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
struct MetroLine {
    metro_line: metro::MetroLine,
}

#[pymethods]
impl MetroLine {
    #[getter]
    fn id(&self) -> u64 {
        self.metro_line.id
    }

    #[getter]
    fn color(&self) -> (u8, u8, u8) {
        self.metro_line.color.into()
    }

    #[getter]
    fn length(&self) -> f64 {
        self.metro_line.length()
    }

    #[getter]
    fn stops(&self) -> Vec<MetroStation> {
        self.metro_line
            .stops()
            .iter()
            .map(|station| station.clone().into())
            .collect()
    }
}

#[pyclass]
#[derive(derive_more::From, derive_more::Into)]
struct MetroKey {
    key: metro::MetroKey,
}

#[pymethods]
impl MetroKey {
    #[staticmethod]
    fn key(x: f64, y: f64) -> Self {
        Self {
            key: metro::MetroKey::Key((x, y).into()),
        }
    }

    #[staticmethod]
    fn stop(x: f64, y: f64, station: &MetroStation) -> Self {
        Self {
            key: metro::MetroKey::Stop((x, y).into(), station.station.clone()),
        }
    }
}

struct PySplineVisitor {
    visitor: PyObject,
}

impl metro::SplineVisitor<PyErr> for PySplineVisitor {
    fn visit(
        &mut self,
        line: &metro::MetroLine,
        vertex: cgmath::Vector2<f64>,
        t: f64,
    ) -> PyResult<()> {
        Python::with_gil(|py| {
            let line = MetroLine::from(line.clone());
            let vertex: (f64, f64) = vertex.into();
            self.visitor.call1(py, (line, vertex, t))?;
            Ok(())
        })
    }
}

#[pymodule]
fn engine(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Config>()?;
    m.add_class::<Address>()?;
    m.add_class::<BranchState>()?;
    m.add_class::<LeafState>()?;
    m.add_class::<VisitData>()?;
    m.add_class::<State>()?;

    m.add_class::<MetroStation>()?;
    m.add_class::<MetroLine>()?;
    m.add_class::<MetroKey>()?;

    return Ok(());
}
