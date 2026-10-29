import numpy as np
from numpy.testing import assert_allclose

import mphlowrs


def test_version():
    assert mphlowrs.__version__ == "0.1.0"


def test_catenary_z_matches_numpy():
    x = np.linspace(-200.0, 200.0, 1001)
    p = 1500.0
    expected = p * (np.cosh(x / p) - 1.0)
    assert_allclose(mphlowrs.catenary_z(x, p), expected, rtol=1e-12)
