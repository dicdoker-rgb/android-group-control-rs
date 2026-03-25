// src/bezier.rs
//
// Human-like gesture simulation using cubic Bézier curves
// Converts simple (x1,y1)→(x2,y2) swipes into 20-30 point paths
// with realistic acceleration/deceleration and slight curvature

use rand::Rng;

/// A point in the gesture path with timing
#[derive(Clone, Debug)]
pub struct GesturePoint {
    pub x: i32,
    pub y: i32,
    pub delay_ms: u32, // delay since previous point
}

/// Generate a human-like gesture path from A to B
/// Returns 20-30 intermediate points with timing
pub fn humanize_swipe(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    total_duration_ms: u32,
) -> Vec<GesturePoint> {
    let mut rng = rand::thread_rng();

    // Number of points (20-30)
    let num_points = rng.gen_range(20..=30);

    // Generate control points for cubic Bézier with slight randomness
    // Add slight curvature (like a human hand arc)
    let dx = (x2 - x1) as f64;
    let dy = (y2 - y1) as f64;
    let dist = (dx * dx + dy * dy).sqrt();

    // Perpendicular offset for natural arc (±5% of distance)
    let perp_offset = dist * rng.gen_range(-0.05..0.05);
    let angle = dy.atan2(dx);
    let perp_x = -angle.sin() * perp_offset;
    let perp_y = angle.cos() * perp_offset;

    // Control points
    let cx1 = x1 as f64 + dx * 0.3 + perp_x + rng.gen_range(-3.0..3.0);
    let cy1 = y1 as f64 + dy * 0.3 + perp_y + rng.gen_range(-3.0..3.0);
    let cx2 = x1 as f64 + dx * 0.7 + perp_x + rng.gen_range(-3.0..3.0);
    let cy2 = y1 as f64 + dy * 0.7 + perp_y + rng.gen_range(-3.0..3.0);

    let mut points = Vec::with_capacity(num_points);

    for i in 0..num_points {
        // Parameter t from 0 to 1
        let t_linear = i as f64 / (num_points - 1) as f64;

        // Apply ease-in-out timing (slow-fast-slow, like real human swipe)
        // Using smoothstep: 3t² - 2t³
        let t = 3.0 * t_linear * t_linear - 2.0 * t_linear * t_linear * t_linear;

        // Cubic Bézier: B(t) = (1-t)³P0 + 3(1-t)²tP1 + 3(1-t)t²P2 + t³P3
        let mt = 1.0 - t;
        let x = mt * mt * mt * x1 as f64
            + 3.0 * mt * mt * t * cx1
            + 3.0 * mt * t * t * cx2
            + t * t * t * x2 as f64;
        let y = mt * mt * mt * y1 as f64
            + 3.0 * mt * mt * t * cy1
            + 3.0 * mt * t * t * cy2
            + t * t * t * y2 as f64;

        // Add micro-jitter (human hand tremor, ±1-2 pixels)
        let jitter_x = rng.gen_range(-1.5..1.5);
        let jitter_y = rng.gen_range(-1.5..1.5);

        // Timing: ease-in-out distribution of total_duration
        let delay = if i == 0 {
            0
        } else {
            let dt = t_linear - ((i - 1) as f64 / (num_points - 1) as f64);
            let base_delay = (dt * total_duration_ms as f64) as u32;
            // Add timing jitter (±20%)
            let jitter = (base_delay as f64 * rng.gen_range(-0.2..0.2)) as i32;
            (base_delay as i32 + jitter).max(5) as u32
        };

        points.push(GesturePoint {
            x: (x + jitter_x).round() as i32,
            y: (y + jitter_y).round() as i32,
            delay_ms: delay,
        });
    }

    points
}

/// Build a shell script that replays bezier gesture points via `input` commands.
/// This sends one `input swipe` per consecutive pair of points, producing a
/// multi-segment humanised gesture over a single `adb shell` invocation.
pub fn build_swipe_shell_script(points: &[GesturePoint]) -> String {
    if points.is_empty() {
        return String::new();
    }
    if points.len() == 1 {
        return format!("input tap {} {}", points[0].x, points[0].y);
    }

    // Build a single shell command that chains short swipes.
    // Each segment: `input swipe x1 y1 x2 y2 delay_ms`
    let mut parts = Vec::with_capacity(points.len() - 1);
    for pair in points.windows(2) {
        let p1 = &pair[0];
        let p2 = &pair[1];
        let dur = p2.delay_ms.max(5);
        parts.push(format!(
            "input swipe {} {} {} {} {}",
            p1.x, p1.y, p2.x, p2.y, dur
        ));
    }
    parts.join(" && ")
}

/// Generate a human-like tap (with slight position variation)
pub fn humanize_tap(x: i32, y: i32) -> (i32, i32) {
    let mut rng = rand::thread_rng();
    // Human finger accuracy is about ±3 pixels
    let dx = rng.gen_range(-3..=3);
    let dy = rng.gen_range(-3..=3);
    (x + dx, y + dy)
}

/// Simulate human typing delay between characters
pub fn typing_delay_ms() -> u32 {
    let mut rng = rand::thread_rng();
    // Human typing: 80-250ms between keystrokes, occasionally 400-600ms (thinking)
    if rng.gen_ratio(1, 10) {
        // 10% chance of a "thinking" pause
        rng.gen_range(400..600)
    } else {
        rng.gen_range(80..250)
    }
}

/// Simulate occasional typo + backspace (returns true if should "mistype")
pub fn should_mistype() -> bool {
    let mut rng = rand::thread_rng();
    rng.gen_ratio(1, 20) // 5% chance of typo
}
