import numpy as np
import pytest
from numpy.testing import assert_allclose

import mphlowrs

span_mod = pytest.importorskip("mechaphlowers.core.models.cable.span")


@pytest.fixture
def catenary_span():
    return span_mod.CatenarySpan(
        span_length=np.array([500.0, 300.0, 480.0, 520.0]),
        elevation_difference=np.array([0.0, 30.0, -12.0, 5.0]),
        parameter=np.array([2000.0, 1500.0, 1800.0, 2100.0]),
        linear_weight=np.float64(9.5),
    )


def test_z_many_points_matches_mechaphlowers(catenary_span):
    x = catenary_span.x(resolution=200)  # (resolution, n_spans)

    expected = catenary_span.z_many_points(x)
    result = mphlowrs.catenary_z_many_points(x, catenary_span.parameter)

    assert result.shape == expected.shape
    assert_allclose(result, expected, rtol=1e-12)


def test_z_many_points_rejects_shape_mismatch(catenary_span):
    x = catenary_span.x(resolution=10)
    with pytest.raises(ValueError, match="columns"):
        mphlowrs.catenary_z_many_points(x, catenary_span.parameter[:-1])
