use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;

/// Version of the compiled extension.
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Catenary sag: z(x) = p * (cosh(x / p) - 1), with p the catenary parameter.
#[pyfunction]
fn catenary_z<'py>(
    py: Python<'py>,
    x: PyReadonlyArray1<'py, f64>,
    p: f64,
) -> Bound<'py, PyArray1<f64>> {
    x.as_array()
        .mapv(|xi| p * ((xi / p).cosh() - 1.0))
        .into_pyarray(py)
}

#[pymodule]
fn _mphlowrs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(catenary_z, m)?)?;
    Ok(())
}
