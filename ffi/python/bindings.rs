use pyo3::prelude::*;

#[pyfunction]
fn foobar() -> i32 {
    return engine::foobar();
}

#[pymodule]
fn engine(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(foobar, m)?)?;
    return Ok(());
}
