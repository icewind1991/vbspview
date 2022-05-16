use splines::Interpolate;

#[derive(Clone, Copy, Default, Debug)]
pub struct Wrapping<const MIN: i32, const MAX: i32>(pub f32);

/// Map input numbers such that the difference between the output % MAX is the wrapping difference between the input
fn unwrap<const MIN: i32, const MAX: i32>(a: f32, b: f32) -> (f32, f32) {
    let offset = (MAX - MIN) as f32;
    if a - b < MIN as f32 {
        (a + offset, b)
    } else if b - a < MIN as f32 {
        (a, b + offset)
    } else {
        (a, b)
    }
}

#[test]
fn test_unwrap() {
    assert_eq!((101.0, 99.0), unwrap::<0, 100>(1.0, 99.0));
    assert_eq!((99.0, 101.0), unwrap::<0, 100>(99.0, 1.0));

    assert_eq!((120.0, 99.0), unwrap::<-100, 100>(-80.0, 99.0));
}

fn wrap<const MIN: i32, const MAX: i32>(num: f32) -> f32 {
    let offset = (MAX - MIN) as f32;
    if num > MAX as f32 {
        num - offset
    } else if num < MIN as f32 {
        num + offset
    } else {
        num
    }
}

impl<const MIN: i32, const MAX: i32> Interpolate<f32> for Wrapping<MIN, MAX> {
    fn step(t: f32, threshold: f32, a: Self, b: Self) -> Self {
        if t < threshold {
            a
        } else {
            b
        }
    }

    fn lerp(t: f32, a: Self, b: Self) -> Self {
        let (a, b) = unwrap::<MIN, MAX>(a.0, b.0);
        let c = f32::lerp(t, a, b);
        Wrapping(wrap::<MIN, MAX>(c))
    }

    fn cosine(t: f32, a: Self, b: Self) -> Self {
        let (a, b) = unwrap::<MIN, MAX>(a.0, b.0);
        let c = f32::cosine(t, a, b);
        Wrapping(wrap::<MIN, MAX>(c))
    }

    fn cubic_hermite(
        _t: f32,
        _x: (f32, Self),
        _a: (f32, Self),
        _b: (f32, Self),
        _y: (f32, Self),
    ) -> Self {
        todo!();
    }

    fn quadratic_bezier(_t: f32, _a: Self, _u: Self, _b: Self) -> Self {
        todo!();
    }

    fn cubic_bezier(_t: f32, _a: Self, _u: Self, _v: Self, _b: Self) -> Self {
        todo!();
    }

    fn cubic_bezier_mirrored(_t: f32, _a: Self, _u: Self, _v: Self, _b: Self) -> Self {
        todo!()
    }
}

#[test]
fn test_wrapping_interp() {
    use splines::{Interpolation, Key, Spline};

    let spline = Spline::from_vec(vec![
        Key::new(0.0, Wrapping::<-180, 180>(160.0), Interpolation::Linear),
        Key::new(10.0, Wrapping::<-180, 180>(-160.0), Interpolation::Linear),
    ]);
    assert_eq!(168.0, spline.sample(2.0).unwrap().0);
    assert_eq!(180.0, spline.sample(5.0).unwrap().0);
    assert_eq!(-172.0, spline.sample(7.0).unwrap().0);
}
