//! End-to-end: init → keyframe edits in sidecars → interpolate → deflicker.

use image::{Rgb, RgbImage};
use std::path::Path;
use std::process::Command;

fn hat_timelapse(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_hat_timelapse"))
        .arg("-p")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn exposure(dir: &Path, name: &str) -> f64 {
    let props = hat_timelapse_xmp_read(&dir.join(name));
    props.trim_start_matches('+').parse().unwrap()
}

fn hat_timelapse_xmp_read(path: &Path) -> String {
    let s = std::fs::read_to_string(path).unwrap();
    let k = s.find("crs:Exposure2012=\"").unwrap() + 18;
    s[k..k + s[k..].find('"').unwrap()].to_string()
}

#[test]
fn edit_loop() {
    let dir = std::env::temp_dir().join(format!("hat_timelapse-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..40u32 {
        // Mid-grey with an alternating flicker.
        let v = if i % 2 == 0 { 120 } else { 104 };
        RgbImage::from_pixel(64, 48, Rgb([v, v, v]))
            .save(dir.join(format!("f_{i:03}.png")))
            .unwrap();
    }
    hat_timelapse(&dir, &["init", "--keyframes", "2"]);

    // "Edit" the two keyframes, as Lightroom or LightCraft would.
    for (name, ev) in [("f_000.xmp", "+1.00"), ("f_039.xmp", "-1.00")] {
        let xml = hat_timelapse_xmp_write(ev);
        std::fs::write(dir.join(name), xml).unwrap();
    }
    hat_timelapse(&dir, &["interpolate", "--method", "linear"]);
    assert!((exposure(&dir, "f_020.xmp") - (-0.03)).abs() < 0.02);

    hat_timelapse(&dir, &["deflicker", "--window", "9"]);
    // Neighbouring frames now differ only by the ramp (~0.05 EV/frame), not the flicker.
    let a = exposure(&dir, "f_020.xmp");
    let b = exposure(&dir, "f_021.xmp");
    assert!(
        (b - a).abs() > 0.2,
        "deflicker should counter the ~0.35 EV flicker: {a} {b}"
    );

    // Re-reading keyframes must recover the user's values, not the deflickered ones.
    hat_timelapse(&dir, &["interpolate"]);
    let p: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("hat_timelapse.json")).unwrap())
            .unwrap();
    let k0 = p["frames"][0]["values"]["Exposure2012"].as_f64().unwrap();
    assert!((k0 - 1.0).abs() < 0.011, "{k0}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn hat_timelapse_xmp_write(ev: &str) -> String {
    format!(
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/" crs:Exposure2012="{ev}"/>
</rdf:RDF></x:xmpmeta>"#
    )
}
